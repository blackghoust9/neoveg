// Adapted from `filmcraft-audio-dsp` `src/effects/mod.rs` (https://github.com/storytold/filmcraft,
// commit 8fda2f2). Copyright (c) 2026 ArtCraft Team and the FilmCraft contributors. MIT OR
// Apache-2.0; Neoveg uses it under the MIT terms, see third_party/filmcraft/LICENSE-MIT.
// Changes: only the effects Neoveg ships are registered; module paths adapted to `neoveg_audio::dsp`.

//! Audio effects and their registry.

pub mod dynamics;
pub mod eq;
pub mod time;

pub use dynamics::{Compressor, Gate, Limiter};
pub use eq::{ParametricEq, SimpleEq};
pub use time::{Delay, Reverb};

use crate::dsp::{AudioEffect, ParamSpec};

/// Effect category (for menus and the effects panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Filter,
    Dynamics,
    Time,
}

impl Category {
    /// Label in the default language (pt-BR).
    pub fn label(self) -> &'static str {
        match self {
            Category::Filter => "Filtros e equalização",
            Category::Dynamics => "Dinâmica",
            Category::Time => "Tempo e espaço",
        }
    }
}

/// Registry entry describing one effect type.
#[derive(Clone, Copy)]
pub struct EffectInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    pub params: &'static [ParamSpec],
    /// `true` when the effect delays the signal (look-ahead). Such an effect would shift one track
    /// against the others, so it is only offered on the master bus.
    pub has_latency: bool,
    /// Construct an instance for `(sample_rate, channels)`.
    pub create: fn(f32, usize) -> Box<dyn AudioEffect>,
}

impl std::fmt::Debug for EffectInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectInfo")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("category", &self.category)
            .field("params", &self.params.len())
            .finish()
    }
}

macro_rules! entry {
    ($id:literal, $name:literal, $cat:ident, $latency:literal, $ty:ty) => {
        EffectInfo {
            id: $id,
            name: $name,
            category: Category::$cat,
            params: <$ty>::PARAMS,
            has_latency: $latency,
            create: |sr, ch| Box::new(<$ty>::new(sr, ch)),
        }
    };
}

static REGISTRY: &[EffectInfo] = &[
    entry!(
        "parametric_eq",
        "Equalizador paramétrico",
        Filter,
        false,
        ParametricEq
    ),
    entry!(
        "simple_eq",
        "Equalizador de 3 bandas",
        Filter,
        false,
        SimpleEq
    ),
    entry!("compressor", "Compressor", Dynamics, false, Compressor),
    entry!("gate", "Expansor / Gate", Dynamics, false, Gate),
    entry!("limiter", "Limitador", Dynamics, true, Limiter),
    entry!("delay", "Delay", Time, false, Delay),
    entry!("reverb", "Reverb", Time, false, Reverb),
];

/// All registered effects.
pub fn effects() -> &'static [EffectInfo] {
    REGISTRY
}

/// Look up an effect by id.
pub fn effect_info(id: &str) -> Option<&'static EffectInfo> {
    REGISTRY.iter().find(|e| e.id == id)
}

/// Instantiate an effect by id.
pub fn create_effect(id: &str, sample_rate: f32, channels: usize) -> Option<Box<dyn AudioEffect>> {
    effect_info(id).map(|e| (e.create)(sample_rate, channels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::testutil::*;
    use crate::dsp::Unit;

    const SR: f32 = 48000.0;

    fn noise(seed: u64, n: usize, amp: f32) -> Vec<f32> {
        let mut r = Rng::new(seed);
        (0..n).map(|_| r.uniform() * amp).collect()
    }

    #[test]
    fn registry_is_consistent() {
        let mut ids = std::collections::HashSet::new();
        for e in effects() {
            assert!(ids.insert(e.id), "duplicate id {}", e.id);
            let fx = (e.create)(SR, 2);
            assert_eq!(fx.id(), e.id);
            assert_eq!(fx.params(), e.params);
            let mut pids = std::collections::HashSet::new();
            for p in e.params {
                assert!(pids.insert(p.id), "{}: duplicate param {}", e.id, p.id);
                assert!(
                    p.min <= p.default && p.default <= p.max,
                    "{}.{}",
                    e.id,
                    p.id
                );
                assert_eq!(fx.param(p.id), Some(p.default));
                if p.unit == Unit::Choice {
                    assert_eq!(p.choices.len() as f32, p.max + 1.0);
                }
            }
        }
        assert!(effect_info("reverb").is_some());
        assert!(create_effect("nope", SR, 2).is_none());
        let mut c = create_effect("compressor", SR, 2).unwrap();
        assert!(!c.set_param("nope", 1.0));
        assert!(c.set_param("ratio", 1000.0));
        assert_eq!(c.param("ratio"), Some(30.0));
        assert!(c.set_param("ratio", f32::NAN));
        assert_eq!(c.param("ratio"), Some(4.0));
    }

    #[test]
    fn only_the_limiter_reports_latency_and_the_flag_says_so() {
        for e in effects() {
            let fx = (e.create)(SR, 2);
            assert_eq!(fx.latency() > 0, e.has_latency, "{}", e.id);
        }
    }

    /// Output is independent of how the stream is cut into blocks.
    #[test]
    fn block_size_independence() {
        let n = 20000;
        let (src_l, src_r) = (noise(1, n, 0.7), noise(2, n, 0.7));
        for e in effects() {
            let run = |size: usize| {
                let mut fx = (e.create)(SR, 2);
                let (mut l, mut r) = (src_l.clone(), src_r.clone());
                let mut pos = 0;
                while pos < n {
                    let end = (pos + size).min(n);
                    fx.process(&mut [&mut l[pos..end], &mut r[pos..end]]);
                    pos = end;
                }
                (l, r)
            };
            let (a, b) = (run(n), run(137));
            let worst =
                a.0.iter()
                    .zip(&b.0)
                    .chain(a.1.iter().zip(&b.1))
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0f32, f32::max);
            assert!(worst < 1e-3, "{} depends on the block size: {worst}", e.id);
        }
    }

    #[test]
    fn every_effect_keeps_silence_silent_and_never_produces_nan() {
        for e in effects() {
            let mut fx = (e.create)(SR, 2);
            let (mut l, mut r) = (vec![0.0f32; 4800], vec![0.0f32; 4800]);
            fx.process(&mut [&mut l, &mut r]);
            assert!(l.iter().chain(&r).all(|v| v.is_finite()), "{}", e.id);
            let (mut l, mut r) = (noise(3, 4800, 1.0), noise(4, 4800, 1.0));
            fx.process(&mut [&mut l, &mut r]);
            assert!(l.iter().chain(&r).all(|v| v.is_finite()), "{}", e.id);
        }
    }
}
