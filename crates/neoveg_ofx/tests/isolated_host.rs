//! The host end to end: a real plug-in library (`neoveg_ofx_sample`, built here) is loaded by a real
//! child process (`neoveg_ofx_host`), and the program is never the one that loads it. The plug-in
//! is also made to crash, to hang and to print, to see that the program only gets an error.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use neoveg_ofx::{describe_in_child, Options};

fn host_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_neoveg_ofx_host"))
}

/// The sample plug-in library, built next to the host program the first time it is needed.
fn sample_library() -> &'static Path {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let dir = host_exe().parent().unwrap().to_path_buf();
        let release = dir.file_name().is_some_and(|n| n == "release");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let mut build = Command::new(cargo);
        build.args(["build", "-p", "neoveg_ofx_sample", "-j", "2"]);
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

fn options(sample: &str, seconds: u64) -> Options {
    Options {
        timeout: Duration::from_secs(seconds),
        env: vec![("NEOVEG_OFX_SAMPLE".into(), sample.into())],
    }
}

#[test]
fn a_plugin_is_described_by_a_child_process() {
    let plugins = describe_in_child(sample_library(), &host_exe(), &Options::default()).unwrap();
    assert_eq!(plugins.len(), 1);
    let p = &plugins[0];
    assert_eq!(p.identifier, "com.neoveg.sample.invert");
    assert_eq!(p.api, "OfxImageEffectPluginAPI");
    assert_eq!(p.version, [1, 2]);
    assert_eq!(p.label, "Neoveg Sample Invert");
    assert_eq!(p.grouping, "Neoveg/Test");
    assert_eq!(p.description, "A plug-in that only describes itself");
    assert_eq!(p.contexts, ["OfxImageEffectContextFilter"]);
    assert_eq!(p.clips, ["Source", "Output"]);
    assert_eq!(p.params.len(), 1);
    assert_eq!(
        (p.params[0].name.as_str(), p.params[0].kind.as_str()),
        ("amount", "OfxParamTypeDouble")
    );
    assert_eq!(p.problem, None);
}

#[test]
fn a_plugin_that_prints_does_not_confuse_the_answer() {
    let plugins =
        describe_in_child(sample_library(), &host_exe(), &options("print_noise", 20)).unwrap();
    assert_eq!(plugins[0].label, "Neoveg Sample Invert");
}

#[test]
fn a_plugin_that_aborts_ends_its_process_and_not_the_program() {
    let error = describe_in_child(sample_library(), &host_exe(), &options("abort_on_load", 20))
        .unwrap_err();
    assert!(error.contains("encerrou"), "{error}");
}

#[test]
fn a_plugin_that_never_answers_is_killed_after_the_time_limit() {
    let started = std::time::Instant::now();
    let error =
        describe_in_child(sample_library(), &host_exe(), &options("hang_on_load", 2)).unwrap_err();
    assert!(error.contains("não respondeu"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "it did not wait for ever"
    );
}

#[test]
fn a_plugin_that_refuses_to_describe_is_reported_with_the_reason() {
    let plugins = describe_in_child(
        sample_library(),
        &host_exe(),
        &options("refuse_describe", 20),
    )
    .unwrap();
    assert!(
        plugins[0]
            .problem
            .as_deref()
            .is_some_and(|p| p.contains("descrever")),
        "{:?}",
        plugins[0]
    );
    assert_eq!(plugins[0].identifier, "com.neoveg.sample.invert");
}

#[test]
fn a_file_that_is_not_a_plugin_library_is_an_error_from_the_child() {
    let dir = std::env::temp_dir().join(format!("neoveg_ofx_notlib_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("not_a_plugin.ofx");
    std::fs::write(&file, b"this is not a library").unwrap();
    let error = describe_in_child(&file, &host_exe(), &Options::default()).unwrap_err();
    assert!(error.contains("não foi possível carregar"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}
