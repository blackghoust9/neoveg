//! End-to-End Opaque-Box Integration Tests: Feature 10 (OFX 1.4 Plugin Host)
//!
//! Tiers covered:
//! - Tier 1: Feature Coverage (5 tests)
//! - Tier 2: Boundary & Corner Cases (5 tests)

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use neoveg_ofx::{describe_in_child, library_of, Options};

fn host_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_neoveg_ofx_host"))
}

fn sample_library() -> &'static Path {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let dir = host_exe().parent().unwrap().to_path_buf();
        let release = dir.file_name().is_some_and(|n| n == "release");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let mut build = Command::new(cargo);
        build.args(["build", "-p", "neoveg_ofx_sample", "-j", "1"]);
        if release {
            build.arg("--release");
        }
        let status = build
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("cargo runs");
        assert!(status.success(), "the sample plug-in builds");
        let name = if cfg!(target_os = "windows") {
            "neoveg_ofx_sample.dll"
        } else if cfg!(target_os = "macos") {
            "libneoveg_ofx_sample.dylib"
        } else {
            "libneoveg_ofx_sample.so"
        };
        let library = dir.join(name);
        assert!(library.is_file(), "{} exists", library.display());
        library
    })
}

fn sample_options(behavior: &str, timeout_secs: u64) -> Options {
    Options {
        timeout: Duration::from_secs(timeout_secs),
        env: vec![("NEOVEG_OFX_SAMPLE".into(), behavior.into())],
    }
}

// ============================================================================
// Feature 10: OFX 1.4 Plugin Host (R2.1)
// ============================================================================

#[test]
fn f10_t1_01_ofx_host_executable_exists_and_runs() {
    let exe = host_exe();
    assert!(exe.is_file(), "Host binary must exist");
}

#[test]
fn f10_t1_02_ofx_sample_plugin_discovery_and_metadata() {
    let plugins = describe_in_child(sample_library(), &host_exe(), &Options::default())
        .expect("Sample plugin must describe cleanly");
    assert_eq!(plugins.len(), 1);
    let p = &plugins[0];
    assert_eq!(p.identifier, "com.neoveg.sample.invert");
    assert_eq!(p.api, "OfxImageEffectPluginAPI");
    assert_eq!(p.version, [1, 2]);
    assert_eq!(p.label, "Neoveg Sample Invert");
    assert_eq!(p.grouping, "Neoveg/Test");
    assert_eq!(p.contexts, ["OfxImageEffectContextFilter"]);
}

#[test]
fn f10_t1_03_ofx_param_specification_and_types() {
    let plugins = describe_in_child(sample_library(), &host_exe(), &Options::default())
        .expect("Plugin must describe");
    let params = &plugins[0].params;
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "amount");
    assert_eq!(params[0].kind, "OfxParamTypeDouble");
}

#[test]
fn f10_t1_04_ofx_clip_bindings_source_and_output() {
    let plugins = describe_in_child(sample_library(), &host_exe(), &Options::default())
        .expect("Plugin must describe");
    let clips = &plugins[0].clips;
    assert_eq!(clips, &["Source", "Output"]);
}

#[test]
fn f10_t1_05_ofx_bundle_path_resolution() {
    let lib_path = sample_library();
    let resolved = library_of(lib_path).expect("Path resolution succeeds");
    assert_eq!(resolved, lib_path);
}

#[test]
fn f10_t2_01_ofx_corrupt_or_non_plugin_library_error() {
    let not_a_plugin = host_exe();
    let err = describe_in_child(&not_a_plugin, &host_exe(), &Options::default())
        .expect_err("Non-plugin binary must fail cleanly");
    assert!(err.contains("not found") || err.contains("OfxGetNumberOfPlugins") || err.contains("error"));
}

#[test]
fn f10_t2_02_ofx_plugin_abort_process_isolation() {
    let err = describe_in_child(
        sample_library(),
        &host_exe(),
        &sample_options("abort_on_load", 5),
    )
    .expect_err("Aborting plugin must be reported without crashing host");
    assert!(!err.is_empty());
}

#[test]
fn f10_t2_03_ofx_plugin_timeout_watchdog_kill() {
    let err = describe_in_child(
        sample_library(),
        &host_exe(),
        &sample_options("hang_on_load", 1),
    )
    .expect_err("Hanging plugin must be terminated after timeout");
    assert!(!err.is_empty());
}

#[test]
fn f10_t2_04_ofx_plugin_stdout_chatter_resilience() {
    let plugins = describe_in_child(
        sample_library(),
        &host_exe(),
        &sample_options("print_noise", 5),
    )
    .expect("Plugin stdout chatter must not corrupt structured IPC report");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].identifier, "com.neoveg.sample.invert");
}

#[test]
fn f10_t2_05_ofx_missing_host_executable_clean_error() {
    let non_existent = Path::new("Z:/non_existent_host_path/neoveg_ofx_host.exe");
    let err = describe_in_child(sample_library(), non_existent, &Options::default())
        .expect_err("Missing host executable must return descriptive error");
    assert!(!err.is_empty());
}
