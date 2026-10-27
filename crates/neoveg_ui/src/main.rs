//! Neoveg entry point: opens the main workspace window with an empty project.

mod app;
mod captions_panel;
mod credits;
mod envelope_canvas;
mod frame_grid;
mod fx_panel;
mod generator_dialog;
mod icons;
mod log_viewer;
mod loudness_panel;
mod mixer_panel;
mod notes_window;
mod precision_slider;
mod project_dialog;
mod render_dialog;
mod scopes_panel;
mod shortcuts;
mod slideshow_window;
mod splash;
mod theme;
mod waveform_paint;
mod welcome;

use eframe::egui;

/// `--screenshots <dir>`: renders the interface offscreen to PNG files and exits.
fn screenshots_dir() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--screenshots" {
            return Some(
                args.next()
                    .unwrap_or_else(|| "docs/screenshots".into())
                    .into(),
            );
        }
    }
    None
}

fn main() -> eframe::Result<()> {
    if let Some(dir) = screenshots_dir() {
        // No window, no log files, none of the user's data: see `app::headless`
        let code = match app::headless::run(&dir) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("erro: {e}");
                1
            }
        };
        std::process::exit(code);
    }
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("logs")))
        .unwrap_or_else(|| "logs".into());
    neoveg_common::logging::init(&log_dir);
    tracing::info!("[INFO] Neoveg starting (pure Rust workspace)");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Neoveg")
            .with_inner_size([1280.0, 720.0])
            .with_maximized(true)
            .with_min_inner_size([800.0, 500.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Neoveg",
        options,
        Box::new(|cc| Ok(Box::new(app::NeovegApp::new(cc)))),
    )
}
