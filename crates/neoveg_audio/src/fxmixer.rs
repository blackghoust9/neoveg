//! Block mixer with effect chains: sums the clips of every track into a bus, runs the track's
//! effects on its bus, sums the buses and runs the master effects.
//!
//! The mixer is built outside the audio callback (it allocates the buses and creates the effects);
//! afterwards `begin`, `add_frame` and `finish` never allocate or lock, so it may live inside the
//! real-time callback. Tracks without effects share one dry bus, so a project without effects
//! costs what the plain per-frame mix cost.

use crate::dsp::effects::{create_effect, effect_info};
use crate::dsp::AudioEffect;
use crate::mixer::{plan_end, MixClip};

/// Most frames the mixer handles per block; the callback cuts longer buffers into blocks.
pub const MAX_BLOCK: usize = 512;

/// One effect to instantiate: its registry id and the parameters to set (bypassed effects are
/// simply left out of the plan by the caller).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectSpec {
    pub kind: String,
    pub params: Vec<(String, f32)>,
}

/// The effect chains of a project: one per track (indexed like `MixClip::track`) and the master's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FxPlan {
    pub tracks: Vec<Vec<EffectSpec>>,
    pub master: Vec<EffectSpec>,
}

impl FxPlan {
    pub fn is_empty(&self) -> bool {
        self.master.is_empty() && self.tracks.iter().all(Vec::is_empty)
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

pub struct BlockMixer {
    clips: Vec<MixClip>,
    /// Bus of every clip (0 is the dry bus).
    clip_bus: Vec<usize>,
    /// Effect chain of every bus; entry 0 (the dry bus) is always empty.
    chains: Vec<Vec<Box<dyn AudioEffect>>>,
    master: Vec<Box<dyn AudioEffect>>,
    bus_l: Vec<Vec<f32>>,
    bus_r: Vec<Vec<f32>>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    end: f64,
}

impl BlockMixer {
    pub fn new(clips: Vec<MixClip>, plan: &FxPlan, sample_rate: f32) -> Self {
        let mut chains: Vec<Vec<Box<dyn AudioEffect>>> = vec![Vec::new()];
        let mut bus_of_track = Vec::with_capacity(plan.tracks.len());
        for specs in &plan.tracks {
            let chain = build_chain(specs, sample_rate);
            if chain.is_empty() {
                bus_of_track.push(0);
            } else {
                bus_of_track.push(chains.len());
                chains.push(chain);
            }
        }
        let clip_bus = clips
            .iter()
            .map(|c| bus_of_track.get(c.track).copied().unwrap_or(0))
            .collect();
        let buses = chains.len();
        Self {
            end: plan_end(&clips),
            clips,
            clip_bus,
            chains,
            master: build_chain(&plan.master, sample_rate),
            bus_l: vec![vec![0.0; MAX_BLOCK]; buses],
            bus_r: vec![vec![0.0; MAX_BLOCK]; buses],
            out_l: vec![0.0; MAX_BLOCK],
            out_r: vec![0.0; MAX_BLOCK],
        }
    }

    /// End of the last clip, seconds.
    pub fn end(&self) -> f64 {
        self.end
    }

    /// Starts a block of `frames` frames (at most [`MAX_BLOCK`]): clears the buses.
    pub fn begin(&mut self, frames: usize) {
        let n = frames.min(MAX_BLOCK);
        for bus in self.bus_l.iter_mut().chain(self.bus_r.iter_mut()) {
            bus[..n].fill(0.0);
        }
    }

    /// Adds every clip's sample at timeline time `t` to frame `i` of the block.
    #[inline]
    pub fn add_frame(&mut self, i: usize, t: f64) {
        for (clip, &bus) in self.clips.iter().zip(&self.clip_bus) {
            let (l, r) = clip.sample_at(t);
            self.bus_l[bus][i] += l;
            self.bus_r[bus][i] += r;
        }
    }

    /// Ends the block: runs every track's effects on its bus, sums the buses and runs the master
    /// effects. The result is in [`out`](Self::out).
    pub fn finish(&mut self, frames: usize) {
        let n = frames.min(MAX_BLOCK);
        for (bus, chain) in self.chains.iter_mut().enumerate().skip(1) {
            let (l, r) = (&mut self.bus_l[bus][..n], &mut self.bus_r[bus][..n]);
            for effect in chain.iter_mut() {
                effect.process(&mut [&mut *l, &mut *r]);
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

    #[test]
    fn the_master_chain_processes_the_sum_of_every_track() {
        // 3.0 of signal into a limiter whose ceiling is -1 dB
        let plan = FxPlan {
            tracks: vec![],
            master: vec![spec("limiter", &[("ceiling", -1.0)])],
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
        };
        // Track 0 has only an unknown effect, track 5 is not in the plan at all
        let clips = vec![constant(0, 0.2, 0.2), constant(5, 0.1, 0.2)];
        let mut mixer = BlockMixer::new(clips, &plan, RATE as f32);
        let (l, _) = render(&mut mixer, 2400, 512);
        assert!(l.iter().all(|v| (v - 0.3).abs() < 1e-6));
        assert!(is_known_effect("reverb") && !is_known_effect("does_not_exist"));
    }

    #[test]
    fn an_empty_plan_is_empty_and_a_plan_with_any_effect_is_not() {
        assert!(FxPlan::default().is_empty());
        assert!(FxPlan {
            tracks: vec![vec![], vec![]],
            master: vec![]
        }
        .is_empty());
        assert!(!FxPlan {
            tracks: vec![vec![], vec![spec("reverb", &[])]],
            master: vec![]
        }
        .is_empty());
    }
}
