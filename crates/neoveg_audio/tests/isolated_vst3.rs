//! The isolated VST3 host end to end: a real plug-in library (`neoveg_vst3_sample`, built here) is
//! opened by a real child process (`neoveg_vst3_host`), and the editor's side only talks to it.
//! The plug-in is also made to crash, to see that the program, the other tracks and the render go on.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use neoveg_audio::dsp::AudioEffect;
use neoveg_audio::fxmixer::{BlockMixer, EffectSpec, FxPlan};
use neoveg_audio::mixer::MixClip;
use neoveg_audio::pcm::PcmStore;
use neoveg_audio::vst3_host::{format_vst3_kind, parse_vst3_kind};
use neoveg_audio::vst3_isolated::{
    crash_of, forget_crash, scan_isolated, IsolatedEffect, SCAN_LIMIT,
};

/// The tests set environment variables of the plug-in and of the program, which are for the whole
/// process: they take turns.
fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn host_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_neoveg_vst3_host"))
}

/// The sample plug-in library, built next to the host program the first time it is needed.
fn sample_library() -> &'static Path {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let dir = host_exe().parent().unwrap().to_path_buf();
        let release = dir.file_name().is_some_and(|n| n == "release");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let mut build = Command::new(cargo);
        build.args(["build", "-p", "neoveg_vst3_sample", "-j", "2"]);
        if release {
            build.arg("--release");
        }
        let status = build
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("cargo runs");
        assert!(status.success(), "the sample plug-in builds");
        let name = if cfg!(target_os = "windows") {
            "neoveg_vst3_sample.dll"
        } else if cfg!(target_os = "macos") {
            "libneoveg_vst3_sample.dylib"
        } else {
            "libneoveg_vst3_sample.so"
        };
        let library = dir.join(name);
        assert!(library.is_file(), "{} exists", library.display());
        library
    })
}

/// The kind of the sample plug-in (found by scanning it).
fn sample_kind() -> String {
    let found = scan_isolated(sample_library(), &host_exe(), SCAN_LIMIT).expect("scan");
    assert_eq!(found.len(), 1);
    found[0].kind.clone()
}

#[test]
fn a_scan_in_a_child_process_finds_the_plugin_its_parameters_and_latency() {
    let _turn = one_at_a_time();
    let found = scan_isolated(sample_library(), &host_exe(), SCAN_LIMIT).unwrap();
    assert_eq!(found.len(), 1);
    let info = &found[0];
    assert_eq!(info.name, "Gain (vst3-rs example plugin)");
    assert_eq!((info.audio_inputs, info.audio_outputs), (2, 2));
    assert_eq!(info.params.len(), 1);
    assert_eq!(info.params[0].name, "Gain");
    assert_eq!(info.latency, 0);
    let (path, class_id) = parse_vst3_kind(&info.kind).unwrap();
    assert_eq!(format_vst3_kind(Path::new(path), class_id), info.kind);
}

#[test]
fn a_file_that_is_not_a_plugin_is_an_error_from_the_scan_not_a_crash() {
    let _turn = one_at_a_time();
    let dir = std::env::temp_dir().join(format!("neoveg_vst3_notlib_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("not_a_plugin.vst3");
    std::fs::write(&file, b"this is not a library").unwrap();
    let error = scan_isolated(&file, &host_exe(), Duration::from_secs(20)).unwrap_err();
    assert!(!error.is_empty(), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_audio_goes_through_the_child_and_the_parameters_reach_the_plugin() {
    let _turn = one_at_a_time();
    std::env::remove_var("NEOVEG_VST3_SAMPLE");
    let kind = sample_kind();
    let mut effect = IsolatedEffect::open(&kind, &host_exe(), 48_000.0).expect("opens");
    assert_eq!(effect.id(), "vst3");
    assert_eq!(effect.latency(), 0);
    let key = effect.dynamic_params()[0].0.clone();

    let mut left = vec![0.5f32; 300];
    let mut right = vec![-0.25f32; 300];
    effect.process(&mut [&mut left, &mut right]);
    assert!(
        (left[10] - 0.5).abs() < 1e-6 && (right[10] + 0.25).abs() < 1e-6,
        "gain 1 passes"
    );

    assert!(effect.set_param(&key, 0.5));
    assert_eq!(effect.param(&key), Some(0.5));
    assert!(!effect.set_param("no-such-parameter", 0.5));
    let mut left = vec![0.5f32; 300];
    let mut right = vec![-0.25f32; 300];
    effect.process(&mut [&mut left, &mut right]);
    assert!(
        (left[0] - 0.25).abs() < 1e-6 && (right[299] + 0.125).abs() < 1e-6,
        "half gain"
    );

    // A block longer than one request is cut and put back together
    let mut long_left = vec![0.5f32; 20_000];
    let mut long_right = vec![0.5f32; 20_000];
    effect.process(&mut [&mut long_left, &mut long_right]);
    assert!(long_left
        .iter()
        .chain(&long_right)
        .all(|s| (s - 0.25).abs() < 1e-6));
    assert!(!effect.has_failed());
}

#[test]
fn the_latency_the_plugin_reports_is_the_one_the_effect_has() {
    let _turn = one_at_a_time();
    std::env::remove_var("NEOVEG_VST3_SAMPLE");
    std::env::set_var("NEOVEG_VST3_SAMPLE_LATENCY", "64");
    let kind = sample_kind();
    let effect = IsolatedEffect::open(&kind, &host_exe(), 48_000.0).expect("opens");
    std::env::remove_var("NEOVEG_VST3_SAMPLE_LATENCY");
    assert_eq!(effect.latency(), 64);
}

#[test]
fn a_plugin_that_crashes_is_noted_and_the_audio_goes_on_delayed_like_the_plugin_would() {
    let _turn = one_at_a_time();
    std::env::remove_var("NEOVEG_VST3_SAMPLE");
    std::env::set_var("NEOVEG_VST3_SAMPLE_LATENCY", "4");
    let kind = sample_kind();
    forget_crash(&kind);
    let mut effect = IsolatedEffect::open(&kind, &host_exe(), 48_000.0).expect("opens");
    std::env::remove_var("NEOVEG_VST3_SAMPLE_LATENCY");
    assert_eq!(effect.latency(), 4);

    // The child dies in the middle of the session: the program does not
    effect.kill_child();
    let mut left: Vec<f32> = (1..=8).map(|n| n as f32).collect();
    let mut right = left.clone();
    effect.process(&mut [&mut left, &mut right]);
    assert!(effect.has_failed());
    assert!(
        crash_of(&kind).is_some(),
        "the failure is noted for the plug-in manager"
    );
    // Four samples of silence, then the input: the same delay the plug-in said it had
    assert_eq!(left, [0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0]);
    assert_eq!(left, right);
    // And it stays so for the next blocks
    let mut more = vec![9.0f32, 9.0];
    effect.process(&mut [&mut more]);
    assert_eq!(more, [5.0, 6.0]);
    forget_crash(&kind);
}

#[test]
fn a_plugin_that_aborts_on_its_first_block_ends_its_process_and_not_the_program() {
    let _turn = one_at_a_time();
    std::env::set_var("NEOVEG_VST3_SAMPLE", "crash_on_process");
    let kind = sample_kind();
    forget_crash(&kind);
    let mut effect = IsolatedEffect::open(&kind, &host_exe(), 48_000.0).expect("opens");
    let mut left = vec![0.5f32; 64];
    let mut right = vec![0.5f32; 64];
    effect.process(&mut [&mut left, &mut right]);
    std::env::remove_var("NEOVEG_VST3_SAMPLE");
    assert!(effect.has_failed());
    assert!(crash_of(&kind).is_some_and(|r| r.contains("terminou") || r.contains("falha")));
    // With no latency, the audio is the input
    assert!(left.iter().all(|s| (s - 0.5).abs() < 1e-6));
    forget_crash(&kind);
}

/// The mixer asks the effect chain for the latency and delays the other tracks to match: the same
/// rule as for every other effect, now with the plug-in in another process.
#[test]
fn the_block_mixer_compensates_the_latency_of_an_isolated_plugin() {
    let _turn = one_at_a_time();
    std::env::remove_var("NEOVEG_VST3_SAMPLE");
    std::env::set_var("NEOVEG_VST3_HOST", host_exe());
    std::env::set_var("NEOVEG_VST3_SAMPLE_LATENCY", "32");
    let kind = sample_kind();
    forget_crash(&kind);

    let clip = |track: usize| MixClip {
        samples: PcmStore::shared(vec![0.5; 4_800 * 2]),
        source_rate: 48_000,
        start: 0.0,
        duration: 0.1,
        gain: 1.0,
        track,
        ..Default::default()
    };
    let plan = FxPlan {
        tracks: vec![
            vec![EffectSpec {
                kind: kind.clone(),
                params: Vec::new(),
            }],
            vec![],
        ],
        ..Default::default()
    };
    let mixer = BlockMixer::new(vec![clip(0), clip(1)], &plan, 48_000.0);
    std::env::remove_var("NEOVEG_VST3_SAMPLE_LATENCY");
    std::env::remove_var("NEOVEG_VST3_HOST");
    assert_eq!(
        mixer.lead_frames(),
        32,
        "the mix lags by the latency of the plug-in"
    );
    forget_crash(&kind);
}
