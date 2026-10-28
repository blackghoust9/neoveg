//! Offscreen screenshots of the interface (`neoveg --screenshots <dir>`), checked end to end.
//!
//! The test starts the real binary, which draws the workspace with egui into a `wgpu` texture (no
//! window, no screen capture) and writes `docs/screenshots/*.png`. It only runs when
//! `NEOVEG_SCREENSHOTS=1`, because it needs a graphics adapter and rewrites tracked files, so a plain
//! `cargo test --workspace` (and the read-only Docker service) skips it with a message:
//!
//! ```text
//! $env:NEOVEG_SCREENSHOTS = "1"; cargo test -p neoveg_ui --test headless_screenshot -- --nocapture
//! ```
//!
//! `NEOVEG_SCREENSHOT_DIR` changes the output folder. With the variable set and no adapter the test
//! fails with the program's message instead of skipping.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

const EXPECTED: [(&str, u32, u32); 12] = [
    ("ui_main_layout.png", 1600, 900),
    ("ui_timeline_events.png", 1400, 520),
    ("ui_multi_project_tabs.png", 1600, 900),
    ("ui_icon_set.png", 1280, 640),
    ("ui_welcome_screen.png", 1280, 720),
    ("ui_project_properties.png", 1280, 860),
    ("ui_edit_details.png", 1400, 700),
    ("ui_video_effects.png", 1600, 900),
    ("ui_pan_crop.png", 1280, 760),
    ("ui_track_motion.png", 1280, 760),
    ("ui_transitions.png", 1600, 900),
    ("ui_velocity.png", 1600, 900),
];

fn output_dir() -> PathBuf {
    std::env::var_os("NEOVEG_SCREENSHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/screenshots"))
}

#[test]
fn the_interface_renders_offscreen_to_png_files() {
    if std::env::var("NEOVEG_SCREENSHOTS").as_deref() != Ok("1") {
        eprintln!(
            "skipped: set NEOVEG_SCREENSHOTS=1 to render docs/screenshots (needs a graphics adapter)"
        );
        return;
    }
    let dir = output_dir();
    std::fs::create_dir_all(&dir).expect("the output folder can be created");
    for (name, _, _) in EXPECTED {
        let _ = std::fs::remove_file(dir.join(name));
    }

    let output = Command::new(env!("CARGO_BIN_EXE_neoveg"))
        .arg("--screenshots")
        .arg(&dir)
        .output()
        .expect("the neoveg binary starts");
    assert!(
        output.status.success(),
        "the screenshot run failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for (name, width, height) in EXPECTED {
        let path = dir.join(name);
        let picture = image::open(&path)
            .unwrap_or_else(|e| panic!("{} is not a readable PNG: {e}", path.display()))
            .to_rgba8();
        assert_eq!(
            (picture.width(), picture.height()),
            (width, height),
            "{name} has the wrong size"
        );
        // A blank or single-colour frame means nothing was drawn
        let colours: HashSet<[u8; 4]> = picture.pixels().map(|p| p.0).take(200_000).collect();
        assert!(
            colours.len() > 30,
            "{name} looks empty: {} distinct colours",
            colours.len()
        );
    }

    // The preview shows a real decoded frame of the sample video (when the clip and `ffmpeg` exist):
    // the test pattern is full of colours, where a black preview has almost none
    let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../neoveg_video/tests/assets/sample_video.mp4");
    let ffmpeg_works = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success());
    if sample.is_file() && ffmpeg_works {
        let main = image::open(dir.join("ui_main_layout.png"))
            .unwrap()
            .to_rgba8();
        let colours: HashSet<[u8; 4]> = (140..460)
            .flat_map(|y| (620..1230).map(move |x| (x, y)))
            .map(|(x, y)| main.get_pixel(x, y).0)
            .collect();
        assert!(
            colours.len() > 300,
            "the preview of ui_main_layout.png has only {} colours: the real frame is missing",
            colours.len()
        );
    }
}
