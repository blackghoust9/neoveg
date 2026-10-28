//! Neoveg entry point: opens the main workspace window with an empty project.

mod app;
mod captions_panel;
mod credits;
mod details_window;
mod dialogs;
mod dock;
mod envelope_canvas;
mod frame_grid;
mod fx_panel;
mod generator_dialog;
mod icons;
mod log_viewer;
mod loudness_panel;
mod mixer_panel;
mod notes_window;
mod pan_crop_window;
mod precision_slider;
mod project_dialog;
mod render_dialog;
mod scopes_panel;
mod shortcuts;
mod slideshow_window;
mod splash;
mod theme;
mod timeline_scroll;
mod transitions_panel;
mod velocity_editor;
mod video_fx_panel;
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

/// The "Nv" mark as the icon of the window (and so of the task bar and the alt-tab list). The same
/// artwork is embedded in the executable by `build.rs`; this one is for the running window.
fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/app_icon/app_icon.png"))
        .unwrap_or_default()
}

fn main() -> eframe::Result<()> {
    if let Some(dir) = screenshots_dir() {
        // No window, no log files, none of the user's data: see `app::headless`
        // A thread with a big stack: the demonstration projects of the ten screenshots live in one
        // function, and a debug build gives every one of them its own stack slot
        let worker = {
            let dir = dir.clone();
            std::thread::Builder::new()
                .name("neoveg-screenshots".into())
                .stack_size(256 * 1024 * 1024)
                .spawn(move || app::headless::run(&dir))
        };
        let result = match worker {
            Ok(handle) => handle
                .join()
                .unwrap_or_else(|_| Err("a captura de tela terminou em pânico".into())),
            Err(e) => Err(format!("não foi possível iniciar a captura de tela: {e}")),
        };
        let code = match result {
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
            .with_icon(app_icon())
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
