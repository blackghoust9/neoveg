//! Block mixer with effect chains: sums the clips of every track into a bus, runs the track's
//! effects on its bus, sums the buses and runs the master effects.
//!
//! The mixer is built outside the audio callback (it allocates the buses and creates the effects);
//! afterwards `begin`, `add_frame` and `finish` never allocate or lock, so it may live inside the
//! real-time callback. Tracks without effects share one dry bus, so a project without effects
//! costs what the plain per-frame mix cost.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::clap_host::{create_clap_effect, is_clap_kind, parse_clap_kind};
use crate::dsp::effects::{create_effect, effect_info};
use crate::dsp::AudioEffect;
use crate::duck::{analyze, DuckCurve, SidechainFollower};
use crate::mixer::{plan_end, MixClip};
use neoveg_common::fade::FadeCurve;
use neoveg_common::pan::PanLaw;

/// Most frames the mixer handles per block; the callback cuts longer buffers into blocks.
pub const MAX_BLOCK: usize = 512;

/// One effect to instantiate: its registry id and the parameters to set (bypassed effects are
/// simply left out of the plan by the caller).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectSpec {
    pub kind: String,
    pub params: Vec<(String, f32)>,
}

/// Auto-ducking of one track: it is turned down by `reduce_db` while the `controllers` tracks
/// (indices like `MixClip::track`) have sound. When `realtime` is `true`, a live sidechain
/// envelope follower reacts to the controllers' post-FX, post-fader signal instead of pre-analysis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DuckSpec {
    pub controllers: Vec<usize>,
    pub sensitivity: f32,
    pub reduce_db: f32,
    pub fade_s: f32,
    pub realtime: bool,
}

/// One keyframe in an audio automation curve (`curve == None` means hold/step until the next key).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutomationKey {
    pub time: f64,
    pub value: f32,
    pub curve: Option<FadeCurve>,
}

/// Sorted keyframe curve evaluated with zero allocations inside the audio callback.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AutomationCurve {
    pub keys: Vec<AutomationKey>,
}

impl AutomationCurve {
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    #[inline]
    pub fn value_at(&self, t: f64) -> Option<f32> {
        let (first, last) = (self.keys.first()?, self.keys.last()?);
        if t <= first.time {
            return Some(first.value);
        }
        if t >= last.time {
            return Some(last.value);
        }
        let idx = self.keys.partition_point(|k| k.time <= t);
        if idx == 0 {
            return Some(first.value);
        }
        if idx >= self.keys.len() {
            return Some(last.value);
        }
        let a = self.keys[idx - 1];
        let b = self.keys[idx];
        let span = b.time - a.time;
        if span <= 1e-6 {
            return Some(b.value);
        }
        let Some(curve) = a.curve else {
            return Some(a.value);
        };
        let u = curve.gain(((t - a.time) / span).clamp(0.0, 1.0)) as f32;
        Some(a.value + (b.value - a.value) * u)
    }
}

/// Automation curve driving a single effect parameter on a track (`Some(track)`) or master (`None`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParamAutomationSpec {
    pub track: Option<usize>,
    pub active_fx_index: usize,
    pub param_id: String,
    pub curve: AutomationCurve,
}

/// The fader, pan and phase of one track, applied after its effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackMixSpec {
    /// Linear gain of the fader.
    pub gain: f32,
    /// `-1.0` (left) to `1.0` (right).
    pub pan: f32,
    pub invert: bool,
}

impl Default for TrackMixSpec {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            invert: false,
        }
    }
}

impl TrackMixSpec {
    pub fn is_neutral(&self) -> bool {
        self.gain == 1.0 && self.pan == 0.0 && !self.invert
    }

    /// Left and right multipliers under `law`, with the polarity flip folded in.
    fn gains(&self, law: PanLaw) -> (f32, f32) {
        let (l, r) = law.gains(self.pan);
        let sign = if self.invert { -1.0 } else { 1.0 };
        (l * self.gain * sign, r * self.gain * sign)
    }
}

/// A send from a track to a user bus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SendSpec {
    /// Index into [`FxPlan::buses`].
    pub bus: usize,
    /// Linear level of the send.
    pub gain: f32,
    /// Taken after the track's fader instead of before it.
    pub post: bool,
    /// Keeps the pan of the track; otherwise the send is a centred stereo signal.
    pub link_pan: bool,
}

/// Where one track goes: the master (`None`) or a user bus, and its sends.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RouteSpec {
    pub output: Option<usize>,
    pub sends: Vec<SendSpec>,
}

impl RouteSpec {
    /// `true` for a track that goes straight to the master with no sends.
    pub fn is_default(&self) -> bool {
        self.output.is_none() && self.sends.is_empty()
    }
}

/// A user bus: it takes what tracks and other buses send it, runs its effects and its fader and
/// goes to the master (`None`) or to another bus.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BusSpec {
    pub chain: Vec<EffectSpec>,
    pub mix: TrackMixSpec,
    pub output: Option<usize>,
}

/// The processing order of the user buses: every bus after the buses that go to it. A bus in a loop
/// has its output ignored (it goes to the master), so a damaged plan cannot hang the mixer.
fn bus_order(buses: &[BusSpec]) -> (Vec<usize>, Vec<Option<usize>>) {
    let n = buses.len();
    // The output of each bus, with loops and bad indices cut
    let mut target: Vec<Option<usize>> =
        buses.iter().map(|b| b.output.filter(|&t| t < n)).collect();
    for start in 0..n {
        let mut seen = vec![false; n];
        let mut at = start;
        while let Some(next) = target[at] {
            if seen[at] {
                target[start] = None;
                break;
            }
            seen[at] = true;
            at = next;
            if at == start {
                target[start] = None;
                break;
            }
        }
    }
    let mut feeders = vec![0usize; n];
    for t in target.iter().flatten() {
        feeders[*t] += 1;
    }
    let mut order = Vec::with_capacity(n);
    let mut ready: Vec<usize> = (0..n).filter(|&i| feeders[i] == 0).collect();
    while let Some(i) = ready.pop() {
        order.push(i);
        if let Some(t) = target[i] {
            feeders[t] -= 1;
            if feeders[t] == 0 {
                ready.push(t);
            }
        }
    }
    (order, target)
}

/// The effect chains of a project: one per track (indexed like `MixClip::track`) and the master's,
/// plus the auto-ducking of the tracks that have it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FxPlan {
    pub tracks: Vec<Vec<EffectSpec>>,
    pub master: Vec<EffectSpec>,
    /// Indexed like `tracks`; may be shorter.
    pub duck: Vec<Option<DuckSpec>>,
    /// Fader, pan and phase of the tracks, indexed like `tracks`; may be shorter.
    pub mix: Vec<TrackMixSpec>,
    pub pan_law: PanLaw,
    /// Optional volume automation curves (in dB, `-60.0..=12.0`), indexed like `tracks`.
    pub track_vol_envelopes: Vec<Option<AutomationCurve>>,
    /// Optional pan automation curves (`-1.0..=1.0`), indexed like `tracks`.
    pub track_pan_envelopes: Vec<Option<AutomationCurve>>,
    /// Effect parameter automation curves for track and master chains.
    pub param_envelopes: Vec<ParamAutomationSpec>,
    /// The user buses (submixes).
    pub buses: Vec<BusSpec>,
    /// Where each track goes and its sends, indexed like `tracks`; may be shorter.
    pub routes: Vec<RouteSpec>,
}

impl FxPlan {
    pub fn is_empty(&self) -> bool {
        self.master.is_empty()
            && self.tracks.iter().all(Vec::is_empty)
            && self
                .duck
                .iter()
                .all(|d| d.as_ref().is_none_or(|d| d.controllers.is_empty()))
            && self.mix.iter().all(TrackMixSpec::is_neutral)
            && self
                .track_vol_envelopes
                .iter()
                .all(|c| c.as_ref().is_none_or(AutomationCurve::is_empty))
            && self
                .track_pan_envelopes
                .iter()
                .all(|c| c.as_ref().is_none_or(AutomationCurve::is_empty))
            && self.param_envelopes.iter().all(|p| p.curve.is_empty())
            && self.buses.is_empty()
            && self.routes.iter().all(RouteSpec::is_default)
    }
}

/// Creates the effect `spec` describes, or `None` for an unknown id. Parameters are clamped to
/// their range by the effect; unknown parameter ids are ignored.
fn build(spec: &EffectSpec, sample_rate: f32) -> Option<Box<dyn AudioEffect>> {
    let mut effect = if is_clap_kind(&spec.kind) {
        create_clap_effect(&spec.kind, sample_rate)?
    } else {
        create_effect(&spec.kind, sample_rate, 2)?
    };
    for (id, value) in &spec.params {
        effect.set_param(id, *value);
    }
    // Start at the target values instead of gliding to them from the defaults
    effect.reset();
    Some(effect)
}

fn build_chain(specs: &[EffectSpec], sample_rate: f32) -> Vec<Box<dyn AudioEffect>> {
    specs
        .iter()
        .filter_map(|spec| {
            let effect = build(spec, sample_rate);
            if effect.is_none() {
                tracing::warn!("[WARN] unknown audio effect `{}` skipped", spec.kind);
            }
            effect
        })
        .collect()
}

/// A fixed delay of whole frames on a stereo bus (delay compensation).
struct FrameDelay {
    left: Vec<f32>,
    right: Vec<f32>,
    pos: usize,
}

impl FrameDelay {
    fn new(frames: usize) -> Self {
        Self {
            left: vec![0.0; frames],
            right: vec![0.0; frames],
            pos: 0,
        }
    }

    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let len = self.left.len();
        if len == 0 {
            return;
        }
        for (l, r) in l.iter_mut().zip(r.iter_mut()) {
            std::mem::swap(l, &mut self.left[self.pos]);
            std::mem::swap(r, &mut self.right[self.pos]);
            self.pos = (self.pos + 1) % len;
        }
    }
}

struct LiveBusSlot {
    track: usize,
    bus: usize,
    gain_l: AtomicU32,
    gain_r: AtomicU32,
    dirty: AtomicBool,
}

struct LiveUserBusSlot {
    bus: usize,
    gain_l: AtomicU32,
    gain_r: AtomicU32,
    dirty: AtomicBool,
}

struct LiveSendSlot {
    track: usize,
    send: usize,
    gain: AtomicU32,
    dirty: AtomicBool,
}

struct LiveParamSlot {
    /// `Some(track)` for a track chain, `None` for the master chain.
    track: Option<usize>,
    /// Bus index (`None` for the master chain).
    bus: Option<usize>,
    /// Index among the instantiated effects in that chain.
    fx_index: usize,
    id: String,
    bits: AtomicU32,
    dirty: AtomicBool,
}

/// Lock-free atomic table allowing the UI thread to glide fader, pan, phase and effect parameters
/// during live playback without allocating or reopening the audio stream.
pub struct LiveControls {
    any_dirty: AtomicBool,
    pan_law: PanLaw,
    buses: Vec<LiveBusSlot>,
    user_buses: Vec<LiveUserBusSlot>,
    sends: Vec<LiveSendSlot>,
    params: Vec<LiveParamSlot>,
}

impl LiveControls {
    /// Updates `track`'s fader, pan and phase multipliers for the next audio block.
    pub fn set_track_mix(&self, track: usize, spec: TrackMixSpec) -> bool {
        let Some(slot) = self.buses.iter().find(|s| s.track == track) else {
            return false;
        };
        let (gl, gr) = spec.gains(self.pan_law);
        slot.gain_l.store(gl.to_bits(), Ordering::Relaxed);
        slot.gain_r.store(gr.to_bits(), Ordering::Relaxed);
        slot.dirty.store(true, Ordering::Relaxed);
        self.any_dirty.store(true, Ordering::Relaxed);
        true
    }

    /// Updates the fader, pan and phase of user bus `bus` (an index into [`FxPlan::buses`]).
    pub fn set_user_bus_mix(&self, bus: usize, spec: TrackMixSpec) -> bool {
        let Some(slot) = self.user_buses.iter().find(|s| s.bus == bus) else {
            return false;
        };
        let (gl, gr) = spec.gains(self.pan_law);
        slot.gain_l.store(gl.to_bits(), Ordering::Relaxed);
        slot.gain_r.store(gr.to_bits(), Ordering::Relaxed);
        slot.dirty.store(true, Ordering::Relaxed);
        self.any_dirty.store(true, Ordering::Relaxed);
        true
    }

    /// Updates the level of the `send`-th send of `track` (linear gain).
    pub fn set_send_gain(&self, track: usize, send: usize, gain: f32) -> bool {
        let Some(slot) = self
            .sends
            .iter()
            .find(|s| s.track == track && s.send == send)
        else {
            return false;
        };
        slot.gain.store(gain.max(0.0).to_bits(), Ordering::Relaxed);
        slot.dirty.store(true, Ordering::Relaxed);
        self.any_dirty.store(true, Ordering::Relaxed);
        true
    }

    /// Updates parameter `param_id` of the `fx_index`-th active effect on `track` (`None` = master).
    pub fn set_param(
        &self,
        track: Option<usize>,
        fx_index: usize,
        param_id: &str,
        value: f32,
    ) -> bool {
        let Some(slot) = self
            .params
            .iter()
            .find(|s| s.track == track && s.fx_index == fx_index && s.id == param_id)
        else {
            return false;
        };
        slot.bits.store(value.to_bits(), Ordering::Relaxed);
        slot.dirty.store(true, Ordering::Relaxed);
        self.any_dirty.store(true, Ordering::Relaxed);
        true
    }
}

struct RealtimeDuckSlot {
    target_bus: usize,
    ctrl_start: usize,
    ctrl_end: usize,
    follower: SidechainFollower,
}

struct BusMixAutoSlot {
    base_gain: f32,
    base_pan: f32,
    invert: bool,
    pan_law: PanLaw,
    vol_curve: Option<AutomationCurve>,
    pan_curve: Option<AutomationCurve>,
}

impl BusMixAutoSlot {
    #[inline]
    fn gains_at(&self, t: f64) -> (f32, f32) {
        let gain = match self.vol_curve.as_ref().and_then(|c| c.value_at(t)) {
            Some(db) if db <= -60.0 => 0.0,
            Some(db) => 10.0_f32.powf(db / 20.0),
            None => self.base_gain,
        };
        let pan = self
            .pan_curve
            .as_ref()
            .and_then(|c| c.value_at(t))
            .unwrap_or(self.base_pan)
            .clamp(-1.0, 1.0);
        let (l, r) = self.pan_law.gains(pan);
        let sign = if self.invert { -1.0 } else { 1.0 };
        (l * gain * sign, r * gain * sign)
    }
}

struct ParamAutoSlot {
    /// `Some(bus)` for a track effect chain, `None` for the master chain.
    bus: Option<usize>,
    fx_index: usize,
    param_id: String,
    curve: AutomationCurve,
}

/// A send of a track's bus, as the mixer mixes it.
struct InternalSend {
    /// User bus it adds to.
    bus: usize,
    /// Level (linear).
    gain: f32,
    post: bool,
    /// Left and right multipliers of a pre-fader send: the pan of the track, or the centre.
    pan: (f32, f32),
}

/// Where the sound of a track's bus goes.
#[derive(Default)]
struct InternalRoute {
    /// A user bus, or `None` for the master.
    target: Option<usize>,
    sends: Vec<InternalSend>,
}

/// A user bus inside the mixer.
struct UserBus {
    chain: Vec<Box<dyn AudioEffect>>,
    /// Left and right multipliers after the effects (fader, pan, phase).
    gain: (f32, f32),
    /// Where it goes: another user bus (processed later) or the master.
    target: Option<usize>,
    l: Vec<f32>,
    r: Vec<f32>,
}

pub struct BlockMixer {
    /// Where the sound of every bus of the tracks goes (entry 0, the dry bus, goes to the master).
    routes: Vec<InternalRoute>,
    /// The user buses and the order they are processed in.
    ubuses: Vec<UserBus>,
    ubus_order: Vec<usize>,
    clips: Vec<MixClip>,
    /// Bus of every clip (0 is the dry bus).
    clip_bus: Vec<usize>,
    /// Effect chain of every bus; entry 0 (the dry bus) is always empty.
    chains: Vec<Vec<Box<dyn AudioEffect>>>,
    /// Auto-ducking curve of every bus (entry 0 never has one) and the buses that have one.
    ducks: Vec<Option<DuckCurve>>,
    ducked: Vec<usize>,
    /// Real-time post-FX / post-fader sidechain followers and flattened controller bus indices.
    realtime_ducks: Vec<RealtimeDuckSlot>,
    sidechain_buses: Vec<usize>,
    /// Left / right multipliers of every bus after its effects (fader, pan, phase).
    bus_mix: Vec<(f32, f32)>,
    /// Optional per-bus volume/pan automation slots (indexed by `bus`).
    bus_mix_auto: Vec<Option<BusMixAutoSlot>>,
    /// Effect parameter automation slots evaluated at the start of each block.
    param_auto: Vec<ParamAutoSlot>,
    /// Gain of every bus at the frame being mixed (1 unless the bus is lookahead-ducked).
    gains: Vec<f32>,
    master: Vec<Box<dyn AudioEffect>>,
    /// Delay that brings every bus to the latency of the slowest one.
    delays: Vec<FrameDelay>,
    /// How many frames the whole mix lags behind the positions fed to it.
    lead: usize,
    bus_l: Vec<Vec<f32>>,
    bus_r: Vec<Vec<f32>>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    end: f64,
    block_t0: f64,
    block_t1: f64,
    block_has_time: bool,
    live: Option<Arc<LiveControls>>,
}

impl BlockMixer {
    pub fn new(clips: Vec<MixClip>, plan: &FxPlan, sample_rate: f32) -> Self {
        Self::build(clips, plan, sample_rate, false).0
    }

    /// Builds a mixer paired with [`LiveControls`] so every audible track gets its own bus and its
    /// fader, pan, phase and effect parameters can be adjusted live from the UI thread.
    pub fn with_live_controls(
        clips: Vec<MixClip>,
        plan: &FxPlan,
        sample_rate: f32,
    ) -> (Self, Arc<LiveControls>) {
        let (mixer, live) = Self::build(clips, plan, sample_rate, true);
        (
            mixer,
            live.expect("live controls requested when live_mode is true"),
        )
    }

    fn build(
        clips: Vec<MixClip>,
        plan: &FxPlan,
        sample_rate: f32,
        live_mode: bool,
    ) -> (Self, Option<Arc<LiveControls>>) {
        let end = plan_end(&clips);
        let mut chains: Vec<Vec<Box<dyn AudioEffect>>> = vec![Vec::new()];
        let mut ducks: Vec<Option<DuckCurve>> = vec![None];
        let mut bus_mix: Vec<(f32, f32)> = vec![(1.0, 1.0)];
        let mut bus_mix_auto: Vec<Option<BusMixAutoSlot>> = vec![None];
        let max_clip_track = clips.iter().map(|c| c.track + 1).max().unwrap_or(0);
        let max_param_track = plan
            .param_envelopes
            .iter()
            .filter_map(|p| p.track.map(|t| t + 1))
            .max()
            .unwrap_or(0);
        let track_count = plan
            .tracks
            .len()
            .max(plan.duck.len())
            .max(plan.mix.len())
            .max(plan.track_vol_envelopes.len())
            .max(plan.track_pan_envelopes.len())
            .max(max_param_track)
            .max(max_clip_track);
        let mut bus_of_track = Vec::with_capacity(track_count);
        let mut routes: Vec<InternalRoute> = vec![InternalRoute::default()];
        let mut live_sends: Vec<LiveSendSlot> = Vec::new();
        let (ubus_order, ubus_target) = bus_order(&plan.buses);
        let mut live_buses = Vec::new();
        let mut live_params = Vec::new();
        for t in 0..track_count {
            let chain = plan
                .tracks
                .get(t)
                .map(|specs| build_chain(specs, sample_rate))
                .unwrap_or_default();
            let duck_spec = plan
                .duck
                .get(t)
                .and_then(Option::as_ref)
                .filter(|d| !d.controllers.is_empty());
            let curve = duck_spec
                .filter(|d| !d.realtime)
                .map(|d| {
                    let controllers: Vec<&MixClip> = clips
                        .iter()
                        .filter(|c| d.controllers.contains(&c.track))
                        .collect();
                    analyze(&controllers, end, d.sensitivity, d.reduce_db, d.fade_s)
                })
                .filter(|c| !c.is_flat());
            let is_rt_ducked = duck_spec.is_some_and(|d| d.realtime);
            let is_rt_controller = plan
                .duck
                .iter()
                .flatten()
                .any(|d| d.realtime && !d.controllers.is_empty() && d.controllers.contains(&t));
            let vol_curve = plan
                .track_vol_envelopes
                .get(t)
                .and_then(Option::as_ref)
                .filter(|c| !c.is_empty())
                .cloned();
            let pan_curve = plan
                .track_pan_envelopes
                .get(t)
                .and_then(Option::as_ref)
                .filter(|c| !c.is_empty())
                .cloned();
            let has_param_auto = plan
                .param_envelopes
                .iter()
                .any(|p| p.track == Some(t) && !p.curve.is_empty());
            let mix = plan.mix.get(t).copied().unwrap_or_default();
            let route = plan.routes.get(t).cloned().unwrap_or_default();
            // A bus that does not exist is read as the master
            let route_output = route.output.filter(|&b| b < plan.buses.len());
            let route_sends: Vec<SendSpec> = route
                .sends
                .iter()
                .filter(|s| s.bus < plan.buses.len())
                .copied()
                .collect();
            let default_route = route_output.is_none() && route_sends.is_empty();
            let has_clips = clips.iter().any(|c| c.track == t);
            let plain = chain.is_empty()
                && curve.is_none()
                && !is_rt_ducked
                && !is_rt_controller
                && vol_curve.is_none()
                && pan_curve.is_none()
                && !has_param_auto
                && default_route;
            if (!live_mode && plain && mix.is_neutral()) || (live_mode && !has_clips && plain) {
                bus_of_track.push(0);
            } else {
                let bus = chains.len();
                let (gl, gr) = mix.gains(plan.pan_law);
                if live_mode {
                    live_buses.push(LiveBusSlot {
                        track: t,
                        bus,
                        gain_l: AtomicU32::new(gl.to_bits()),
                        gain_r: AtomicU32::new(gr.to_bits()),
                        dirty: AtomicBool::new(false),
                    });
                    for (fx_index, fx) in chain.iter().enumerate() {
                        for spec in fx.params() {
                            let val = fx.param(spec.id).unwrap_or(spec.default);
                            live_params.push(LiveParamSlot {
                                track: Some(t),
                                bus: Some(bus),
                                fx_index,
                                id: spec.id.to_string(),
                                bits: AtomicU32::new(val.to_bits()),
                                dirty: AtomicBool::new(false),
                            });
                        }
                        for (dyn_id, dyn_default) in fx.dynamic_params() {
                            let val = fx.param(dyn_id).unwrap_or(*dyn_default);
                            live_params.push(LiveParamSlot {
                                track: Some(t),
                                bus: Some(bus),
                                fx_index,
                                id: dyn_id.clone(),
                                bits: AtomicU32::new(val.to_bits()),
                                dirty: AtomicBool::new(false),
                            });
                        }
                    }
                }
                let mix_auto =
                    (vol_curve.is_some() || pan_curve.is_some()).then_some(BusMixAutoSlot {
                        base_gain: mix.gain,
                        base_pan: mix.pan,
                        invert: mix.invert,
                        pan_law: plan.pan_law,
                        vol_curve,
                        pan_curve,
                    });
                bus_of_track.push(bus);
                chains.push(chain);
                ducks.push(curve);
                bus_mix.push((gl, gr));
                bus_mix_auto.push(mix_auto);
                let centre = plan.pan_law.gains(0.0);
                let own_pan = plan.pan_law.gains(mix.pan);
                if live_mode {
                    for (send_index, send) in route_sends.iter().enumerate() {
                        live_sends.push(LiveSendSlot {
                            track: t,
                            send: send_index,
                            gain: AtomicU32::new(send.gain.to_bits()),
                            dirty: AtomicBool::new(false),
                        });
                    }
                }
                routes.push(InternalRoute {
                    target: route_output,
                    sends: route_sends
                        .iter()
                        .map(|send| InternalSend {
                            bus: send.bus,
                            gain: send.gain,
                            post: send.post,
                            pan: if send.link_pan { own_pan } else { centre },
                        })
                        .collect(),
                });
            }
        }
        let mut realtime_ducks = Vec::new();
        let mut sidechain_buses = Vec::new();
        for (t, maybe_duck) in plan.duck.iter().enumerate() {
            let Some(d) = maybe_duck
                .as_ref()
                .filter(|d| d.realtime && !d.controllers.is_empty())
            else {
                continue;
            };
            let target_bus = bus_of_track.get(t).copied().unwrap_or(0);
            if target_bus == 0 {
                continue;
            }
            let ctrl_start = sidechain_buses.len();
            for &ctrl_track in &d.controllers {
                if let Some(&cbus) = bus_of_track.get(ctrl_track) {
                    if cbus != 0
                        && cbus != target_bus
                        && !sidechain_buses[ctrl_start..].contains(&cbus)
                    {
                        sidechain_buses.push(cbus);
                    }
                }
            }
            let ctrl_end = sidechain_buses.len();
            if ctrl_end > ctrl_start {
                realtime_ducks.push(RealtimeDuckSlot {
                    target_bus,
                    ctrl_start,
                    ctrl_end,
                    follower: SidechainFollower::new(
                        sample_rate,
                        d.sensitivity,
                        d.reduce_db,
                        d.fade_s,
                    ),
                });
            }
        }
        let param_auto: Vec<ParamAutoSlot> = plan
            .param_envelopes
            .iter()
            .filter(|p| !p.curve.is_empty())
            .filter_map(|p| {
                let bus = match p.track {
                    Some(t) => Some(*bus_of_track.get(t).filter(|&&b| b != 0)?),
                    None => None,
                };
                Some(ParamAutoSlot {
                    bus,
                    fx_index: p.active_fx_index,
                    param_id: p.param_id.clone(),
                    curve: p.curve.clone(),
                })
            })
            .collect();
        let clip_bus = clips
            .iter()
            .map(|c| bus_of_track.get(c.track).copied().unwrap_or(0))
            .collect();
        let latency =
            |chain: &[Box<dyn AudioEffect>]| chain.iter().map(|e| e.latency()).sum::<usize>();
        let mut live_user_buses = Vec::new();
        let ubuses: Vec<UserBus> = plan
            .buses
            .iter()
            .enumerate()
            .map(|(i, spec)| {
                let (gl, gr) = spec.mix.gains(plan.pan_law);
                if live_mode {
                    live_user_buses.push(LiveUserBusSlot {
                        bus: i,
                        gain_l: AtomicU32::new(gl.to_bits()),
                        gain_r: AtomicU32::new(gr.to_bits()),
                        dirty: AtomicBool::new(false),
                    });
                }
                UserBus {
                    chain: build_chain(&spec.chain, sample_rate),
                    gain: (gl, gr),
                    target: ubus_target[i],
                    l: vec![0.0; MAX_BLOCK],
                    r: vec![0.0; MAX_BLOCK],
                }
            })
            .collect();
        // Latency from a user bus to the master: its own effects plus the buses it goes through
        let mut route_latency = vec![0usize; ubuses.len()];
        for &i in ubus_order.iter().rev() {
            route_latency[i] =
                latency(&ubuses[i].chain) + ubuses[i].target.map_or(0, |t| route_latency[t]);
        }
        let bus_latency: Vec<usize> = chains
            .iter()
            .zip(&routes)
            .map(|(c, r)| latency(c) + r.target.map_or(0, |t| route_latency[t]))
            .collect();
        let slowest = bus_latency.iter().copied().max().unwrap_or(0);
        let delays = bus_latency
            .iter()
            .map(|l| FrameDelay::new(slowest - l))
            .collect();
        let master = build_chain(&plan.master, sample_rate);
        if live_mode {
            for (fx_index, fx) in master.iter().enumerate() {
                for spec in fx.params() {
                    let val = fx.param(spec.id).unwrap_or(spec.default);
                    live_params.push(LiveParamSlot {
                        track: None,
                        bus: None,
                        fx_index,
                        id: spec.id.to_string(),
                        bits: AtomicU32::new(val.to_bits()),
                        dirty: AtomicBool::new(false),
                    });
                }
                for (dyn_id, dyn_default) in fx.dynamic_params() {
                    let val = fx.param(dyn_id).unwrap_or(*dyn_default);
                    live_params.push(LiveParamSlot {
                        track: None,
                        bus: None,
                        fx_index,
                        id: dyn_id.clone(),
                        bits: AtomicU32::new(val.to_bits()),
                        dirty: AtomicBool::new(false),
                    });
                }
            }
        }
        let lead = slowest + latency(&master);
        let buses = chains.len();
        let live = live_mode.then(|| {
            Arc::new(LiveControls {
                any_dirty: AtomicBool::new(false),
                pan_law: plan.pan_law,
                buses: live_buses,
                user_buses: live_user_buses,
                sends: live_sends,
                params: live_params,
            })
        });
        (
            Self {
                end,
                clips,
                clip_bus,
                ducked: (0..ducks.len()).filter(|&b| ducks[b].is_some()).collect(),
                realtime_ducks,
                sidechain_buses,
                gains: vec![1.0; buses],
                bus_mix,
                bus_mix_auto,
                param_auto,
                ducks,
                chains,
                master,
                delays,
                lead,
                bus_l: vec![vec![0.0; MAX_BLOCK]; buses],
                bus_r: vec![vec![0.0; MAX_BLOCK]; buses],
                out_l: vec![0.0; MAX_BLOCK],
                out_r: vec![0.0; MAX_BLOCK],
                block_t0: 0.0,
                block_t1: 0.0,
                block_has_time: false,
                live: live.clone(),
                routes,
                ubuses,
                ubus_order,
            },
            live,
        )
    }

    /// The internal bus that holds the sends of `track`, if it has any.
    fn send_owner(&self, track: usize) -> Option<usize> {
        self.live
            .as_ref()?
            .buses
            .iter()
            .find(|s| s.track == track)
            .map(|s| s.bus)
    }

    /// End of the last clip, seconds.
    pub fn end(&self) -> f64 {
        self.end
    }

    /// Frames by which the mix lags behind the positions it is fed: the longest effect chain of a
    /// track plus the master's. Feed the mixer positions that many frames ahead of what should
    /// be heard (after [`prime`](Self::prime)) and everything lines up.
    pub fn lead_frames(&self) -> usize {
        self.lead
    }

    /// Runs [`lead_frames`](Self::lead_frames) frames from `start` and throws the result away, so
    /// that the first frame heard afterwards is the mix at `start`.
    pub fn prime(&mut self, start: f64, step: f64) {
        let mut done = 0;
        while done < self.lead {
            let n = (self.lead - done).min(MAX_BLOCK);
            self.begin(n);
            for i in 0..n {
                self.add_frame(i, start + (done + i) as f64 * step);
            }
            self.finish(n);
            done += n;
        }
    }

    /// Starts a block of `frames` frames (at most [`MAX_BLOCK`]): clears the buses and pulls any
    /// lock-free live parameter or fader updates queued by the UI thread.
    pub fn begin(&mut self, frames: usize) {
        let n = frames.min(MAX_BLOCK);
        self.block_has_time = false;
        for bus in self.bus_l.iter_mut().chain(self.bus_r.iter_mut()) {
            bus[..n].fill(0.0);
        }
        for ubus in &mut self.ubuses {
            ubus.l[..n].fill(0.0);
            ubus.r[..n].fill(0.0);
        }
        let Some(live) = &self.live else {
            return;
        };
        if !live.any_dirty.swap(false, Ordering::Relaxed) {
            return;
        }
        for slot in &live.buses {
            if slot.dirty.swap(false, Ordering::Relaxed) {
                let gl = f32::from_bits(slot.gain_l.load(Ordering::Relaxed));
                let gr = f32::from_bits(slot.gain_r.load(Ordering::Relaxed));
                if let Some(entry) = self.bus_mix.get_mut(slot.bus) {
                    *entry = (gl, gr);
                }
            }
        }
        for slot in &live.user_buses {
            if slot.dirty.swap(false, Ordering::Relaxed) {
                let gl = f32::from_bits(slot.gain_l.load(Ordering::Relaxed));
                let gr = f32::from_bits(slot.gain_r.load(Ordering::Relaxed));
                if let Some(ubus) = self.ubuses.get_mut(slot.bus) {
                    ubus.gain = (gl, gr);
                }
            }
        }
        for slot in &live.sends {
            if slot.dirty.swap(false, Ordering::Relaxed) {
                let gain = f32::from_bits(slot.gain.load(Ordering::Relaxed));
                // The track's bus is the one that has this track's sends
                if let Some(bus) = self.send_owner(slot.track) {
                    if let Some(send) = self.routes[bus].sends.get_mut(slot.send) {
                        send.gain = gain;
                    }
                }
            }
        }
        for slot in &live.params {
            if slot.dirty.swap(false, Ordering::Relaxed) {
                let value = f32::from_bits(slot.bits.load(Ordering::Relaxed));
                let fx = match slot.bus {
                    Some(bus) => self
                        .chains
                        .get_mut(bus)
                        .and_then(|c| c.get_mut(slot.fx_index)),
                    None => self.master.get_mut(slot.fx_index),
                };
                if let Some(fx) = fx {
                    fx.set_param(&slot.id, value);
                }
            }
        }
    }

    /// Adds every clip's sample at timeline time `t` to frame `i` of the block.
    #[inline]
    pub fn add_frame(&mut self, i: usize, t: f64) {
        if !self.block_has_time {
            self.block_t0 = t;
            self.block_has_time = true;
        }
        self.block_t1 = t;
        for &bus in &self.ducked {
            if let Some(curve) = &self.ducks[bus] {
                self.gains[bus] = curve.gain(t);
            }
        }
        for (clip, &bus) in self.clips.iter().zip(&self.clip_bus) {
            let (l, r) = clip.sample_at(t);
            let gain = self.gains[bus];
            self.bus_l[bus][i] += l * gain;
            self.bus_r[bus][i] += r * gain;
        }
    }

    /// Ends the block: runs every track's effects on its bus, applies latency alignment, fader/pan
    /// automation and real-time sidechain ducking, sums the buses and runs the master effects.
    pub fn finish(&mut self, frames: usize) {
        let n = frames.min(MAX_BLOCK);
        if self.block_has_time {
            for slot in &self.param_auto {
                let Some(val) = slot.curve.value_at(self.block_t0) else {
                    continue;
                };
                let fx = match slot.bus {
                    Some(bus) => self
                        .chains
                        .get_mut(bus)
                        .and_then(|c| c.get_mut(slot.fx_index)),
                    None => self.master.get_mut(slot.fx_index),
                };
                if let Some(fx) = fx {
                    fx.set_param(&slot.param_id, val);
                }
            }
        }
        for (bus, chain) in self.chains.iter_mut().enumerate() {
            let (l, r) = (&mut self.bus_l[bus][..n], &mut self.bus_r[bus][..n]);
            for effect in chain.iter_mut() {
                effect.process(&mut [&mut *l, &mut *r]);
            }
            self.delays[bus].process(l, r);
            // Pre-fader sends: a copy of the sound after the effects, whatever the fader says
            for send in self.routes[bus].sends.iter().filter(|s| !s.post) {
                let ubus = &mut self.ubuses[send.bus];
                for i in 0..n {
                    ubus.l[i] += l[i] * send.gain * send.pan.0;
                    ubus.r[i] += r[i] * send.gain * send.pan.1;
                }
            }
            if let Some(auto_mix) = self
                .bus_mix_auto
                .get(bus)
                .and_then(Option::as_ref)
                .filter(|_| self.block_has_time && n > 0)
            {
                let (gl0, gr0) = auto_mix.gains_at(self.block_t0);
                let (gl1, gr1) = auto_mix.gains_at(self.block_t1);
                let inv = if n > 1 { 1.0 / (n - 1) as f32 } else { 0.0 };
                for (i, (ls, rs)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
                    let frac = i as f32 * inv;
                    *ls *= gl0 + (gl1 - gl0) * frac;
                    *rs *= gr0 + (gr1 - gr0) * frac;
                }
            } else {
                let (gl, gr) = self.bus_mix[bus];
                if (gl, gr) != (1.0, 1.0) {
                    for (l, r) in l.iter_mut().zip(r.iter_mut()) {
                        *l *= gl;
                        *r *= gr;
                    }
                }
            }
            // Post-fader sends follow the fader (and the pan already in the sound)
            for send in self.routes[bus].sends.iter().filter(|s| s.post) {
                let ubus = &mut self.ubuses[send.bus];
                for i in 0..n {
                    ubus.l[i] += l[i] * send.gain;
                    ubus.r[i] += r[i] * send.gain;
                }
            }
        }
        for slot in &mut self.realtime_ducks {
            for i in 0..n {
                let (mut sl, mut sr) = (0.0_f32, 0.0_f32);
                for &cbus in &self.sidechain_buses[slot.ctrl_start..slot.ctrl_end] {
                    sl += self.bus_l[cbus][i];
                    sr += self.bus_r[cbus][i];
                }
                let g = slot.follower.next_gain(0.5 * (sl * sl + sr * sr));
                self.bus_l[slot.target_bus][i] *= g;
                self.bus_r[slot.target_bus][i] *= g;
            }
        }
        self.out_l[..n].fill(0.0);
        self.out_r[..n].fill(0.0);
        // The buses of the tracks go to the master or to a user bus
        for bus in 0..self.chains.len() {
            match self.routes[bus].target {
                None => {
                    for i in 0..n {
                        self.out_l[i] += self.bus_l[bus][i];
                        self.out_r[i] += self.bus_r[bus][i];
                    }
                }
                Some(j) => {
                    let ubus = &mut self.ubuses[j];
                    for i in 0..n {
                        ubus.l[i] += self.bus_l[bus][i];
                        ubus.r[i] += self.bus_r[bus][i];
                    }
                }
            }
        }
        // The user buses, each after the ones that feed it
        for oi in 0..self.ubus_order.len() {
            let j = self.ubus_order[oi];
            let (mut l, mut r) = (
                std::mem::take(&mut self.ubuses[j].l),
                std::mem::take(&mut self.ubuses[j].r),
            );
            for effect in self.ubuses[j].chain.iter_mut() {
                effect.process(&mut [&mut l[..n], &mut r[..n]]);
            }
            let (gl, gr) = self.ubuses[j].gain;
            for i in 0..n {
                l[i] *= gl;
                r[i] *= gr;
            }
            match self.ubuses[j].target {
                None => {
                    for i in 0..n {
                        self.out_l[i] += l[i];
                        self.out_r[i] += r[i];
                    }
                }
                Some(k) => {
                    for i in 0..n {
                        self.ubuses[k].l[i] += l[i];
                        self.ubuses[k].r[i] += r[i];
                    }
                }
            }
            self.ubuses[j].l = l;
            self.ubuses[j].r = r;
        }
        for effect in self.master.iter_mut() {
            effect.process(&mut [&mut self.out_l[..n], &mut self.out_r[..n]]);
        }
    }

    /// The mixed block (left, right) after [`finish`](Self::finish).
    pub fn out(&self) -> (&[f32], &[f32]) {
        (&self.out_l, &self.out_r)
    }
}

/// `true` when the registry knows a built-in effect with this id or `kind` is a well-formed CLAP
/// plug-in identifier (`"clap:<path>::<plugin_id>"`).
pub fn is_known_effect(kind: &str) -> bool {
    effect_info(kind).is_some() || parse_clap_kind(kind).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const RATE: u32 = 48_000;

    /// `seconds` of a constant stereo value starting at 0 on `track`.
    fn constant(track: usize, value: f32, seconds: f64) -> MixClip {
        MixClip {
            samples: Arc::new(vec![value; (RATE as f64 * seconds) as usize * 2]),
            source_rate: RATE,
            start: 0.0,
            duration: seconds,
            gain: 1.0,
            track,
            ..Default::default()
        }
    }

    /// Renders `frames` frames in blocks of `block` frames.
    fn render(mixer: &mut BlockMixer, frames: usize, block: usize) -> (Vec<f32>, Vec<f32>) {
        let step = 1.0 / f64::from(RATE);
        let (mut l, mut r) = (Vec::new(), Vec::new());
        let mut done = 0;
        while done < frames {
            let n = block.min(frames - done).min(MAX_BLOCK);
            mixer.begin(n);
            for i in 0..n {
                mixer.add_frame(i, (done + i) as f64 * step);
            }
            mixer.finish(n);
            let (ol, or) = mixer.out();
            l.extend_from_slice(&ol[..n]);
            r.extend_from_slice(&or[..n]);
            done += n;
        }
        (l, r)
    }

    fn spec(kind: &str, params: &[(&str, f32)]) -> EffectSpec {
        EffectSpec {
            kind: kind.into(),
            params: params.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect(),
        }
    }

    #[test]
    fn without_effects_the_output_is_the_plain_sum_of_the_clips() {
        let clips = vec![constant(0, 0.25, 0.5), constant(1, 0.125, 0.5)];
        let mut mixer = BlockMixer::new(clips, &FxPlan::default(), RATE as f32);
        let (l, r) = render(&mut mixer, 4800, 512);
        assert!(l.iter().chain(&r).all(|v| (v - 0.375).abs() < 1e-6));
        assert!((mixer.end() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_track_effect_changes_only_its_own_track() {
        // Track 1 goes through a 100 % wet 10 ms delay; track 0 stays dry
        let plan = FxPlan {
            tracks: vec![
                vec![],
                vec![spec(
                    "delay",
                    &[("time", 10.0), ("feedback", 0.0), ("mix", 100.0)],
                )],
            ],
            master: vec![],
            ..Default::default()
        };
        let clips = vec![constant(0, 0.25, 1.0), constant(1, 0.5, 1.0)];
        let mut mixer = BlockMixer::new(clips, &plan, RATE as f32);
        let (l, _) = render(&mut mixer, RATE as usize / 2, 480);

        // Right at the start only the dry track is there (the delayed one has not arrived)
        assert!((l[10] - 0.25).abs() < 0.01, "start: {}", l[10]);
        // Later both are: 0.25 dry + 0.5 delayed
        let settled = l[RATE as usize / 4];
        assert!((settled - 0.75).abs() < 0.02, "settled: {settled}");
    }

    fn mix_plan(mix: Vec<TrackMixSpec>, pan_law: PanLaw) -> FxPlan {
        FxPlan {
            tracks: vec![vec![]; mix.len()],
            mix,
            pan_law,
            ..Default::default()
        }
    }

    #[test]
    fn a_neutral_mixer_keeps_the_plan_empty_and_the_sound_unchanged() {
        let plan = mix_plan(vec![TrackMixSpec::default(); 2], PanLaw::ConstantPower);
        assert!(plan.is_empty());
        let mut mixer = BlockMixer::new(
            vec![constant(0, 0.25, 0.2), constant(1, 0.5, 0.2)],
            &plan,
            RATE as f32,
        );
        let (l, r) = render(&mut mixer, 480, 480);
        assert!((l[100] - 0.75).abs() < 1e-6 && (r[100] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn the_fader_scales_only_its_own_track() {
        let half = TrackMixSpec {
            gain: 0.5,
            ..Default::default()
        };
        let plan = mix_plan(vec![TrackMixSpec::default(), half], PanLaw::Balance);
        assert!(!plan.is_empty());
        let mut mixer = BlockMixer::new(
            vec![constant(0, 0.2, 0.2), constant(1, 0.4, 0.2)],
            &plan,
            RATE as f32,
        );
        let (l, _) = render(&mut mixer, 480, 480);
        assert!((l[100] - 0.4).abs() < 1e-6, "{}", l[100]);
    }

    #[test]
    fn pan_moves_a_track_between_the_channels() {
        let hard_right = TrackMixSpec {
            pan: 1.0,
            ..Default::default()
        };
        let plan = mix_plan(vec![hard_right], PanLaw::Balance);
        let mut mixer = BlockMixer::new(vec![constant(0, 0.5, 0.2)], &plan, RATE as f32);
        let (l, r) = render(&mut mixer, 480, 480);
        assert!(l[100].abs() < 1e-6 && (r[100] - 0.5).abs() < 1e-6);
        let half = TrackMixSpec {
            pan: 0.0,
            gain: 1.0,
            invert: false,
        };
        let linear = mix_plan(vec![half, hard_right], PanLaw::Linear);
        let mut mixer = BlockMixer::new(vec![constant(1, 0.5, 0.2)], &linear, RATE as f32);
        let (l, r) = render(&mut mixer, 480, 480);
        assert!(l[100].abs() < 1e-6 && (r[100] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn inverting_the_phase_cancels_against_the_original() {
        let flip = TrackMixSpec {
            invert: true,
            ..Default::default()
        };
        let plan = mix_plan(vec![TrackMixSpec::default(), flip], PanLaw::Balance);
        let mut mixer = BlockMixer::new(
            vec![constant(0, 0.4, 0.2), constant(1, 0.4, 0.2)],
            &plan,
            RATE as f32,
        );
        let (l, r) = render(&mut mixer, 480, 480);
        assert!(l[100].abs() < 1e-6 && r[100].abs() < 1e-6);
    }

    #[test]
    fn the_master_chain_processes_the_sum_of_every_track() {
        // 3.0 of signal into a limiter whose ceiling is -1 dB
        let plan = FxPlan {
            tracks: vec![],
            master: vec![spec("limiter", &[("ceiling", -1.0)])],
            ..Default::default()
        };
        let clips = vec![constant(0, 1.5, 1.0), constant(1, 1.5, 1.0)];
        let mut mixer = BlockMixer::new(clips, &plan, RATE as f32);
        let (l, r) = render(&mut mixer, RATE as usize / 2, 512);
        let ceiling = 10f32.powf(-1.0 / 20.0);
        let peak = l[4800..]
            .iter()
            .chain(&r[4800..])
            .fold(0.0f32, |a, v| a.max(v.abs()));
        assert!(peak <= ceiling + 0.01, "limiter let {peak} through");
        assert!(peak > 0.5, "the limiter should not mute the signal: {peak}");
    }

    #[test]
    fn the_result_does_not_depend_on_how_the_stream_is_cut_into_blocks() {
        let plan = FxPlan {
            tracks: vec![vec![spec(
                "delay",
                &[("time", 7.0), ("feedback", 40.0), ("mix", 50.0)],
            )]],
            master: vec![spec("compressor", &[("threshold", -30.0), ("ratio", 8.0)])],
            ..Default::default()
        };
        let clips = || vec![constant(0, 0.4, 0.3)];
        let mut a = BlockMixer::new(clips(), &plan, RATE as f32);
        let mut b = BlockMixer::new(clips(), &plan, RATE as f32);
        let (la, _) = render(&mut a, 9600, 512);
        let (lb, _) = render(&mut b, 9600, 97);
        let worst = la
            .iter()
            .zip(&lb)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 1e-3, "{worst}");
    }

    #[test]
    fn unknown_effects_are_skipped_and_a_clip_on_an_unplanned_track_stays_dry() {
        let plan = FxPlan {
            tracks: vec![vec![spec("does_not_exist", &[])]],
            master: vec![spec("also_missing", &[])],
            ..Default::default()
        };
        // Track 0 has only an unknown effect, track 5 is not in the plan at all
        let clips = vec![constant(0, 0.2, 0.2), constant(5, 0.1, 0.2)];
        let mut mixer = BlockMixer::new(clips, &plan, RATE as f32);
        let (l, _) = render(&mut mixer, 2400, 512);
        assert!(l.iter().all(|v| (v - 0.3).abs() < 1e-6));
        assert!(is_known_effect("reverb") && !is_known_effect("does_not_exist"));
    }

    /// Renders the way the audio callback does: primes the mixer, then feeds it positions that
    /// many frames ahead, so output frame `j` is the mix at `j / RATE` seconds.
    fn render_aligned(mixer: &mut BlockMixer, frames: usize) -> Vec<f32> {
        let step = 1.0 / f64::from(RATE);
        mixer.prime(0.0, step);
        let lead = mixer.lead_frames();
        let mut out = Vec::new();
        let mut done = 0;
        while done < frames {
            let n = MAX_BLOCK.min(frames - done);
            mixer.begin(n);
            for i in 0..n {
                mixer.add_frame(i, (lead + done + i) as f64 * step);
            }
            mixer.finish(n);
            out.extend_from_slice(&mixer.out().0[..n]);
            done += n;
        }
        out
    }

    /// A clip that starts at `start` seconds with a constant value.
    fn step_at(track: usize, value: f32, start: f64) -> MixClip {
        MixClip {
            start,
            ..constant(track, value, 1.0 - start)
        }
    }

    #[test]
    fn an_effect_with_latency_does_not_shift_its_track_against_the_others() {
        // Two identical steps at 0.2 s; only track 1 goes through a look-ahead limiter
        let plan = FxPlan {
            tracks: vec![vec![], vec![spec("limiter", &[("ceiling", 0.0)])]],
            master: vec![],
            ..Default::default()
        };
        let clips = vec![step_at(0, 0.3, 0.2), step_at(1, 0.3, 0.2)];
        let mut mixer = BlockMixer::new(clips, &plan, RATE as f32);
        assert!(mixer.lead_frames() > 0, "the limiter looks ahead");
        let out = render_aligned(&mut mixer, RATE as usize / 2);

        let step = (0.2 * f64::from(RATE)) as usize;
        assert!(
            out[step - 4].abs() < 1e-3,
            "before the step: {}",
            out[step - 4]
        );
        // Both tracks switch on together, so the sum goes straight to 0.6 (no 0.3 plateau)
        assert!(
            (out[step + 8] - 0.6).abs() < 0.02,
            "just after: {}",
            out[step + 8]
        );
    }

    #[test]
    fn the_frame_delay_shifts_a_stream_by_exactly_its_length_across_blocks() {
        let mut delay = FrameDelay::new(3);
        let input: Vec<f32> = (1..=10).map(|v| v as f32).collect();
        let (mut l, mut r) = (input.clone(), input.clone());
        // Cut in uneven blocks
        for range in [0..4, 4..5, 5..10] {
            delay.process(&mut l[range.clone()], &mut r[range]);
        }
        assert_eq!(l, [0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        assert_eq!(l, r);
        // No delay is a no-op
        let mut none = FrameDelay::new(0);
        let mut x = vec![1.0f32, 2.0];
        let mut y = x.clone();
        none.process(&mut x, &mut y);
        assert_eq!(x, [1.0, 2.0]);
    }

    #[test]
    fn the_master_chain_latency_is_part_of_the_lead() {
        let plan = FxPlan {
            tracks: vec![],
            master: vec![spec("limiter", &[])],
            ..Default::default()
        };
        let mut mixer = BlockMixer::new(vec![step_at(0, 0.3, 0.2)], &plan, RATE as f32);
        assert!(mixer.lead_frames() > 0);
        let out = render_aligned(&mut mixer, RATE as usize / 2);
        let step = (0.2 * f64::from(RATE)) as usize;
        assert!(out[step - 4].abs() < 1e-3);
        assert!((out[step + 8] - 0.3).abs() < 0.02, "{}", out[step + 8]);
    }

    #[test]
    fn a_project_without_latency_has_no_lead() {
        let mixer = BlockMixer::new(vec![constant(0, 0.1, 0.1)], &FxPlan::default(), RATE as f32);
        assert_eq!(mixer.lead_frames(), 0);
    }

    /// `value` from `start` for `seconds` on `track`.
    fn burst(track: usize, value: f32, start: f64, seconds: f64) -> MixClip {
        MixClip {
            start,
            ..constant(track, value, seconds)
        }
    }

    fn duck_plan(controllers: Vec<usize>, sensitivity: f32) -> FxPlan {
        FxPlan {
            tracks: vec![vec![], vec![]],
            master: vec![],
            duck: vec![
                None,
                Some(DuckSpec {
                    controllers,
                    sensitivity,
                    reduce_db: 12.0,
                    fade_s: 0.2,
                    realtime: false,
                }),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn music_is_turned_down_while_the_voice_speaks_and_comes_back() {
        // Voice on track 0 from 1 s to 2 s; music on track 1 for 3 s
        let clips = vec![burst(0, 0.3, 1.0, 1.0), constant(1, 0.5, 3.0)];
        let mut mixer = BlockMixer::new(clips, &duck_plan(vec![0], 6.0), RATE as f32);
        let (l, _) = render(&mut mixer, RATE as usize * 3, 480);
        let at = |seconds: f64| l[(seconds * f64::from(RATE)) as usize];

        assert!((at(0.5) - 0.5).abs() < 1e-3, "before: {}", at(0.5));
        let ducked = 0.3 + 0.5 * 10f32.powf(-12.0 / 20.0);
        assert!(
            (at(1.5) - ducked).abs() < 0.01,
            "during: {} vs {ducked}",
            at(1.5)
        );
        assert!((at(2.8) - 0.5).abs() < 1e-3, "after: {}", at(2.8));
        // The music starts coming down before the voice is there
        assert!(at(0.9) < 0.49, "the ramp begins early: {}", at(0.9));
    }

    #[test]
    fn ducking_without_usable_controllers_changes_nothing() {
        let clips = || vec![burst(0, 0.3, 1.0, 1.0), constant(1, 0.5, 3.0)];
        for plan in [duck_plan(vec![], 6.0), duck_plan(vec![7], 6.0)] {
            let mut mixer = BlockMixer::new(clips(), &plan, RATE as f32);
            let (l, _) = render(&mut mixer, RATE as usize * 2, 480);
            assert!((l[(1.5 * f64::from(RATE)) as usize] - 0.8).abs() < 1e-3);
        }
        // A voice too quiet for the sensitivity does not duck either
        let quiet = vec![burst(0, 0.001, 1.0, 1.0), constant(1, 0.5, 3.0)];
        let mut mixer = BlockMixer::new(quiet, &duck_plan(vec![0], 0.0), RATE as f32);
        let (l, _) = render(&mut mixer, RATE as usize * 2, 480);
        assert!((l[(1.5 * f64::from(RATE)) as usize] - 0.501).abs() < 1e-3);
    }

    #[test]
    fn a_ducked_track_keeps_its_effects_and_only_that_track_is_ducked() {
        // The voice itself is never ducked, and the music still goes through its own chain
        let plan = FxPlan {
            tracks: vec![
                vec![],
                vec![spec(
                    "delay",
                    &[("time", 10.0), ("feedback", 0.0), ("mix", 0.0)],
                )],
            ],
            duck: duck_plan(vec![0], 6.0).duck,
            ..Default::default()
        };
        let clips = vec![burst(0, 0.3, 1.0, 1.0), constant(1, 0.5, 3.0)];
        let mut mixer = BlockMixer::new(clips, &plan, RATE as f32);
        let (l, _) = render(&mut mixer, RATE as usize * 2, 480);
        let during = l[(1.5 * f64::from(RATE)) as usize];
        let ducked = 0.3 + 0.5 * 10f32.powf(-12.0 / 20.0);
        assert!((during - ducked).abs() < 0.02, "{during} vs {ducked}");
    }

    #[test]
    fn an_empty_plan_is_empty_and_a_plan_with_any_effect_is_not() {
        assert!(FxPlan::default().is_empty());
        assert!(FxPlan {
            tracks: vec![vec![], vec![]],
            master: vec![],
            ..Default::default()
        }
        .is_empty());
        assert!(!duck_plan(vec![0], 6.0).is_empty());
        assert!(
            duck_plan(vec![], 6.0).is_empty(),
            "ducking without controllers does nothing"
        );
        assert!(!FxPlan {
            tracks: vec![vec![], vec![spec("reverb", &[])]],
            master: vec![],
            ..Default::default()
        }
        .is_empty());
    }

    #[test]
    fn live_controls_update_track_mix_and_effect_parameters_between_blocks() {
        let plan = FxPlan {
            tracks: vec![vec![spec("amplify", &[("gain", 0.0)])]],
            master: vec![spec("amplify", &[("gain", 0.0)])],
            ..Default::default()
        };
        let (mut mixer, live) =
            BlockMixer::with_live_controls(vec![constant(0, 0.5, 4.0)], &plan, RATE as f32);
        let (l0, r0) = render(&mut mixer, 256, 256);
        assert!((l0[200] - 0.5).abs() < 1e-4 && (r0[200] - 0.5).abs() < 1e-4);

        // Hard-pan right and invert phase live without rebuilding the mixer
        assert!(live.set_track_mix(
            0,
            TrackMixSpec {
                gain: 1.0,
                pan: 1.0,
                invert: true,
            },
        ));
        // Boost master amplify by +6.0206 dB (~2x); let the 20 ms smoother settle
        assert!(live.set_param(None, 0, "gain", 6.0206));
        mixer.begin(512);
        for i in 0..512 {
            mixer.add_frame(i, 0.5 + i as f64 / f64::from(RATE));
        }
        mixer.finish(512);
        mixer.begin(512);
        for i in 0..512 {
            mixer.add_frame(i, 1.0 + i as f64 / f64::from(RATE));
        }
        mixer.finish(512);
        let (l1, r1) = mixer.out();
        assert!(l1[500].abs() < 1e-4, "left hard-panned silent: {}", l1[500]);
        assert!(
            (r1[500] - (-1.0)).abs() < 0.02,
            "right inverted and +6 dB boosted: {}",
            r1[500]
        );
    }

    #[test]
    fn realtime_sidechain_reacts_to_live_controller_fader_and_automation_curves_drive_mix_and_fx() {
        // Track 0: key signal (constant 0.4); Track 1: music (constant 0.5) ducked by 12 dB in realtime
        let plan = FxPlan {
            tracks: vec![vec![], vec![spec("amplify", &[("gain", 0.0)])]],
            duck: vec![
                None,
                Some(DuckSpec {
                    controllers: vec![0],
                    sensitivity: 6.0,
                    reduce_db: 12.0,
                    fade_s: 0.1,
                    realtime: true,
                }),
            ],
            track_vol_envelopes: vec![
                None,
                Some(AutomationCurve {
                    keys: vec![
                        AutomationKey {
                            time: 0.0,
                            value: 0.0,
                            curve: Some(FadeCurve::Linear),
                        },
                        AutomationKey {
                            time: 2.0,
                            value: -6.0206,
                            curve: Some(FadeCurve::Linear),
                        },
                    ],
                }),
            ],
            param_envelopes: vec![ParamAutomationSpec {
                track: Some(1),
                active_fx_index: 0,
                param_id: "gain".into(),
                curve: AutomationCurve {
                    keys: vec![
                        AutomationKey {
                            time: 0.0,
                            value: 0.0,
                            curve: None,
                        },
                        AutomationKey {
                            time: 1.5,
                            value: 6.0206,
                            curve: None,
                        },
                    ],
                },
            }],
            ..Default::default()
        };
        let clips = vec![constant(0, 0.4, 3.0), constant(1, 0.5, 3.0)];
        let (mut mixer, live) = BlockMixer::with_live_controls(clips, &plan, RATE as f32);

        // Render first 0.5 s: track 0 (0.4) keys real-time ducking on track 1 (~0.5 * 0.2512)
        let (l0, _) = render(&mut mixer, RATE as usize / 2, 256);
        let expected_ducked =
            0.4 + 0.5 * 10.0_f32.powf(-12.0 / 20.0) * 10.0_f32.powf(-1.505 / 20.0);
        assert!(
            (*l0.last().unwrap() - expected_ducked).abs() < 0.04,
            "ducked sum: {} vs {expected_ducked}",
            l0.last().unwrap()
        );

        // Pull controller track 0 fader down to 0.0 live -> sidechain releases!
        assert!(live.set_track_mix(
            0,
            TrackMixSpec {
                gain: 0.0,
                pan: 0.0,
                invert: false,
            },
        ));
        let step = 1.0 / f64::from(RATE);
        let mut last = 0.0;
        let start_frame = RATE as usize / 2;
        // Advance past 2.0 s: amplify env boosts +6 dB while vol env attenuates -6 dB -> net ~0.5!
        let frames_more = RATE as usize * 2;
        let mut done = 0;
        while done < frames_more {
            let n = 256.min(frames_more - done);
            mixer.begin(n);
            for i in 0..n {
                mixer.add_frame(i, (start_frame + done + i) as f64 * step);
            }
            mixer.finish(n);
            last = mixer.out().0[n - 1];
            done += n;
        }
        assert!(
            (last - 0.5).abs() < 0.03,
            "sidechain released and +6 dB FX / -6 dB vol envelopes cancelled to 0.5: {last}"
        );
    }

    #[test]
    fn clap_plugin_in_block_mixer_receives_latency_compensation_and_live_param_updates() {
        let clap_kind = crate::clap_host::tests::BUILTIN_TEST_KIND;
        // Track 0 has a 16-sample-latency CLAP gain plug-in set to 2.0x; Track 1 is dry (-1.0).
        // With a pulse on both tracks at frame 32, PDC must align Track 1 to Track 0's 16-sample
        // latency so both arrive at output frame 32 + 16 = 48!
        let mut pulse_pos = vec![0.0_f32; 256 * 2];
        pulse_pos[32 * 2] = 0.5;
        pulse_pos[32 * 2 + 1] = 0.5;
        let mut pulse_neg = vec![0.0_f32; 256 * 2];
        pulse_neg[32 * 2] = -1.0;
        pulse_neg[32 * 2 + 1] = -1.0;

        let clips = vec![
            MixClip {
                samples: Arc::new(pulse_pos),
                source_rate: RATE,
                start: 0.0,
                duration: 256.0 / f64::from(RATE),
                gain: 1.0,
                track: 0,
                ..Default::default()
            },
            MixClip {
                samples: Arc::new(pulse_neg),
                source_rate: RATE,
                start: 0.0,
                duration: 256.0 / f64::from(RATE),
                gain: 1.0,
                track: 1,
                ..Default::default()
            },
        ];
        let plan = FxPlan {
            tracks: vec![vec![spec(clap_kind, &[("1", 2.0)])], vec![]],
            ..Default::default()
        };
        let (mut mixer, live) = BlockMixer::with_live_controls(clips, &plan, RATE as f32);
        assert_eq!(mixer.lead_frames(), 16);

        // Prime + render 128 frames: 0.5 * 2.0 (CLAP) + (-1.0) (dry) must cancel to 0 everywhere!
        mixer.prime(0.0, 1.0 / f64::from(RATE));
        let step = 1.0 / f64::from(RATE);
        mixer.begin(128);
        for i in 0..128 {
            mixer.add_frame(i, (16 + i) as f64 * step);
        }
        mixer.finish(128);
        let max_err = mixer.out().0[..128]
            .iter()
            .copied()
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        assert!(
            max_err < 1e-5,
            "PDC failed to align CLAP track with dry track: max_err={max_err}"
        );

        // Live parameter update works on the dynamic CLAP parameter ID `"1"`.
        assert!(live.set_param(Some(0), 0, "1", 3.0));
    }

    // ---- buses and sends ---------------------------------------------------------------------

    fn fader(gain: f32) -> TrackMixSpec {
        TrackMixSpec {
            gain,
            ..Default::default()
        }
    }

    fn bus(gain: f32, output: Option<usize>) -> BusSpec {
        BusSpec {
            chain: Vec::new(),
            mix: fader(gain),
            output,
        }
    }

    fn to(output: Option<usize>, sends: Vec<SendSpec>) -> RouteSpec {
        RouteSpec { output, sends }
    }

    fn level(plan: FxPlan, clips: Vec<MixClip>) -> f32 {
        let mut mixer = BlockMixer::new(clips, &plan, RATE as f32);
        let (l, r) = render(&mut mixer, 2400, 512);
        // Settled value: both channels agree and are constant after the start
        assert!((l[2000] - r[2000]).abs() < 1e-6);
        l[2000]
    }

    #[test]
    fn a_track_sent_to_a_bus_is_heard_through_the_fader_of_the_bus() {
        let plan = FxPlan {
            buses: vec![bus(0.5, None)],
            routes: vec![to(Some(0), vec![])],
            ..Default::default()
        };
        assert!((level(plan, vec![constant(0, 0.4, 0.5)]) - 0.2).abs() < 1e-6);
        // Without the route it is the plain track
        assert!((level(FxPlan::default(), vec![constant(0, 0.4, 0.5)]) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn two_tracks_on_one_bus_share_its_fader_and_a_track_off_it_does_not() {
        let plan = FxPlan {
            buses: vec![bus(0.5, None)],
            routes: vec![
                to(Some(0), vec![]),
                to(Some(0), vec![]),
                RouteSpec::default(),
            ],
            ..Default::default()
        };
        let clips = vec![
            constant(0, 0.2, 0.5),
            constant(1, 0.2, 0.5),
            constant(2, 0.1, 0.5),
        ];
        // (0.2 + 0.2) * 0.5 + 0.1
        assert!((level(plan, clips) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_bus_can_go_through_another_bus_to_the_master() {
        // Bus 0 goes to bus 1, which goes to the master; each takes half
        let plan = FxPlan {
            buses: vec![bus(0.5, Some(1)), bus(0.5, None)],
            routes: vec![to(Some(0), vec![])],
            ..Default::default()
        };
        assert!((level(plan, vec![constant(0, 0.8, 0.5)]) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn a_pre_fader_send_ignores_the_fader_and_a_post_fader_send_follows_it() {
        let send = |post| SendSpec {
            bus: 0,
            gain: 1.0,
            post,
            link_pan: false,
        };
        // The track's own fader is down to 0.25 and it goes to the master; the bus is at unity
        let make = |post| FxPlan {
            mix: vec![fader(0.25)],
            buses: vec![bus(1.0, None)],
            routes: vec![to(None, vec![send(post)])],
            ..Default::default()
        };
        // Main path 0.25 * 0.8 = 0.2, plus the send: pre 0.8, post 0.25 * 0.8 = 0.2
        let clips = || vec![constant(0, 0.8, 0.5)];
        assert!((level(make(false), clips()) - (0.2 + 0.8)).abs() < 1e-6);
        assert!((level(make(true), clips()) - (0.2 + 0.2)).abs() < 1e-6);
    }

    #[test]
    fn a_send_carries_its_own_level_and_a_track_can_go_only_to_the_bus() {
        let plan = FxPlan {
            buses: vec![bus(1.0, None)],
            routes: vec![to(
                Some(0),
                vec![SendSpec {
                    bus: 0,
                    gain: 0.5,
                    post: false,
                    link_pan: false,
                }],
            )],
            ..Default::default()
        };
        // Output to the bus (0.6) and the send at half level into the same bus (0.3)
        assert!((level(plan, vec![constant(0, 0.6, 0.5)]) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn a_loop_of_buses_in_a_damaged_plan_cannot_hang_the_mixer_and_a_missing_bus_is_the_master() {
        let looped = FxPlan {
            buses: vec![bus(1.0, Some(1)), bus(1.0, Some(0))],
            routes: vec![to(Some(0), vec![])],
            ..Default::default()
        };
        let v = level(looped, vec![constant(0, 0.5, 0.5)]);
        assert!(v.is_finite() && v > 0.0, "the sound still comes out: {v}");
        // A route to a bus that is not in the plan reads as the master
        let missing = FxPlan {
            routes: vec![to(
                Some(7),
                vec![SendSpec {
                    bus: 9,
                    gain: 1.0,
                    post: false,
                    link_pan: false,
                }],
            )],
            ..Default::default()
        };
        assert!((level(missing, vec![constant(0, 0.5, 0.5)]) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn the_effects_of_a_bus_work_on_what_it_sums_and_its_latency_is_compensated() {
        use crate::dsp::effects::create_effect;
        let limiter_latency = create_effect("limiter", RATE as f32, 2).map_or(0, |e| e.latency());
        if limiter_latency == 0 {
            return;
        }
        let through_bus = FxPlan {
            buses: vec![BusSpec {
                chain: vec![spec("limiter", &[])],
                mix: fader(1.0),
                output: None,
            }],
            routes: vec![to(Some(0), vec![]), RouteSpec::default()],
            ..Default::default()
        };
        let mixer = BlockMixer::new(
            vec![constant(0, 0.2, 0.5), constant(1, 0.2, 0.5)],
            &through_bus,
            RATE as f32,
        );
        // The mix lags by the limiter's latency, and the track that skips the bus is delayed to match
        assert_eq!(mixer.lead_frames(), limiter_latency);
        let direct = BlockMixer::new(
            vec![constant(0, 0.2, 0.5), constant(1, 0.2, 0.5)],
            &FxPlan::default(),
            RATE as f32,
        );
        assert_eq!(direct.lead_frames(), 0);
    }

    #[test]
    fn live_controls_change_a_bus_fader_and_a_send_level_during_playback() {
        let plan = FxPlan {
            buses: vec![bus(1.0, None)],
            routes: vec![to(
                None,
                vec![SendSpec {
                    bus: 0,
                    gain: 1.0,
                    post: false,
                    link_pan: false,
                }],
            )],
            ..Default::default()
        };
        let (mut mixer, live) =
            BlockMixer::with_live_controls(vec![constant(0, 0.5, 0.5)], &plan, RATE as f32);
        // Main 0.5 plus the send 0.5
        let (l, _) = render(&mut mixer, 1200, 512);
        assert!((l[1000] - 1.0).abs() < 1e-6);
        assert!(live.set_user_bus_mix(0, fader(0.5)));
        assert!(live.set_send_gain(0, 0, 0.5));
        assert!(!live.set_user_bus_mix(5, fader(1.0)), "no such bus");
        assert!(!live.set_send_gain(0, 3, 1.0), "no such send");
        let (l, _) = render(&mut mixer, 1200, 512);
        // Main 0.5 plus the send 0.5 * 0.5 (level) * 0.5 (bus fader)
        assert!((l[1000] - 0.625).abs() < 1e-6, "{}", l[1000]);
    }
}
