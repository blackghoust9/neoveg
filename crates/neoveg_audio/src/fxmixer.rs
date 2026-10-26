//! Block mixer with effect chains: sums the clips of every track into a bus, runs the track's
//! effects on its bus, sums the buses and runs the master effects.
//!
//! The mixer is built outside the audio callback (it allocates the buses and creates the effects);
//! afterwards `begin`, `add_frame` and `finish` never allocate or lock, so it may live inside the
//! real-time callback. Tracks without effects share one dry bus, so a project without effects
//! costs what the plain per-frame mix cost.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::dsp::effects::{create_effect, effect_info};
use crate::dsp::AudioEffect;
use crate::duck::{analyze, DuckCurve};
use crate::mixer::{plan_end, MixClip};
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
/// (indices like `MixClip::track`) have sound.
#[derive(Debug, Clone, PartialEq)]
pub struct DuckSpec {
    pub controllers: Vec<usize>,
    pub sensitivity: f32,
    pub reduce_db: f32,
    pub fade_s: f32,
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
    }
}

/// Creates the effect `spec` describes, or `None` for an unknown id. Parameters are clamped to
/// their range by the effect; unknown parameter ids are ignored.
fn build(spec: &EffectSpec, sample_rate: f32) -> Option<Box<dyn AudioEffect>> {
    let mut effect = create_effect(&spec.kind, sample_rate, 2)?;
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

struct LiveParamSlot {
    /// `Some(track)` for a track chain, `None` for the master chain.
    track: Option<usize>,
    /// Bus index (`None` for the master chain).
    bus: Option<usize>,
    /// Index among the instantiated effects in that chain.
    fx_index: usize,
    id: &'static str,
    bits: AtomicU32,
    dirty: AtomicBool,
}

/// Lock-free atomic table allowing the UI thread to glide fader, pan, phase and effect parameters
/// during live playback without allocating or reopening the audio stream.
pub struct LiveControls {
    any_dirty: AtomicBool,
    pan_law: PanLaw,
    buses: Vec<LiveBusSlot>,
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

pub struct BlockMixer {
    clips: Vec<MixClip>,
    /// Bus of every clip (0 is the dry bus).
    clip_bus: Vec<usize>,
    /// Effect chain of every bus; entry 0 (the dry bus) is always empty.
    chains: Vec<Vec<Box<dyn AudioEffect>>>,
    /// Auto-ducking curve of every bus (entry 0 never has one) and the buses that have one.
    ducks: Vec<Option<DuckCurve>>,
    ducked: Vec<usize>,
    /// Left / right multipliers of every bus after its effects (fader, pan, phase).
    bus_mix: Vec<(f32, f32)>,
    /// Gain of every bus at the frame being mixed (1 unless the bus is ducked).
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
        let max_clip_track = clips.iter().map(|c| c.track + 1).max().unwrap_or(0);
        let track_count = plan
            .tracks
            .len()
            .max(plan.duck.len())
            .max(plan.mix.len())
            .max(max_clip_track);
        let mut bus_of_track = Vec::with_capacity(track_count);
        let mut live_buses = Vec::new();
        let mut live_params = Vec::new();
        for t in 0..track_count {
            let chain = plan
                .tracks
                .get(t)
                .map(|specs| build_chain(specs, sample_rate))
                .unwrap_or_default();
            let curve = plan
                .duck
                .get(t)
                .and_then(Option::as_ref)
                .filter(|d| !d.controllers.is_empty())
                .map(|d| {
                    let controllers: Vec<&MixClip> = clips
                        .iter()
                        .filter(|c| d.controllers.contains(&c.track))
                        .collect();
                    analyze(&controllers, end, d.sensitivity, d.reduce_db, d.fade_s)
                })
                .filter(|c| !c.is_flat());
            let mix = plan.mix.get(t).copied().unwrap_or_default();
            let has_clips = clips.iter().any(|c| c.track == t);
            if !live_mode && chain.is_empty() && curve.is_none() && mix.is_neutral() {
                bus_of_track.push(0);
            } else if live_mode && !has_clips && chain.is_empty() && curve.is_none() {
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
                                id: spec.id,
                                bits: AtomicU32::new(val.to_bits()),
                                dirty: AtomicBool::new(false),
                            });
                        }
                    }
                }
                bus_of_track.push(bus);
                chains.push(chain);
                ducks.push(curve);
                bus_mix.push((gl, gr));
            }
        }
        let clip_bus = clips
            .iter()
            .map(|c| bus_of_track.get(c.track).copied().unwrap_or(0))
            .collect();
        let latency =
            |chain: &[Box<dyn AudioEffect>]| chain.iter().map(|e| e.latency()).sum::<usize>();
        let bus_latency: Vec<usize> = chains.iter().map(|c| latency(c)).collect();
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
                        id: spec.id,
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
                params: live_params,
            })
        });
        (
            Self {
                end,
                clips,
                clip_bus,
                ducked: (0..ducks.len()).filter(|&b| ducks[b].is_some()).collect(),
                gains: vec![1.0; buses],
                bus_mix,
                ducks,
                chains,
                master,
                delays,
                lead,
                bus_l: vec![vec![0.0; MAX_BLOCK]; buses],
                bus_r: vec![vec![0.0; MAX_BLOCK]; buses],
                out_l: vec![0.0; MAX_BLOCK],
                out_r: vec![0.0; MAX_BLOCK],
                live: live.clone(),
            },
            live,
        )
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
        for bus in self.bus_l.iter_mut().chain(self.bus_r.iter_mut()) {
            bus[..n].fill(0.0);
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
                    fx.set_param(slot.id, value);
                }
            }
        }
    }

    /// Adds every clip's sample at timeline time `t` to frame `i` of the block.
    #[inline]
    pub fn add_frame(&mut self, i: usize, t: f64) {
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

    /// Ends the block: runs every track's effects on its bus, sums the buses and runs the master
    /// effects. The result is in [`out`](Self::out).
    pub fn finish(&mut self, frames: usize) {
        let n = frames.min(MAX_BLOCK);
        for (bus, chain) in self.chains.iter_mut().enumerate() {
            let (l, r) = (&mut self.bus_l[bus][..n], &mut self.bus_r[bus][..n]);
            for effect in chain.iter_mut() {
                effect.process(&mut [&mut *l, &mut *r]);
            }
            self.delays[bus].process(l, r);
            let (gl, gr) = self.bus_mix[bus];
            if (gl, gr) != (1.0, 1.0) {
                for (l, r) in l.iter_mut().zip(r.iter_mut()) {
                    *l *= gl;
                    *r *= gr;
                }
            }
        }
        self.out_l[..n].copy_from_slice(&self.bus_l[0][..n]);
        self.out_r[..n].copy_from_slice(&self.bus_r[0][..n]);
        for bus in 1..self.chains.len() {
            for i in 0..n {
                self.out_l[i] += self.bus_l[bus][i];
                self.out_r[i] += self.bus_r[bus][i];
            }
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

/// `true` when the registry knows an effect with this id.
pub fn is_known_effect(kind: &str) -> bool {
    effect_info(kind).is_some()
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
}
