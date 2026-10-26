//! Neoveg entry point: opens the main workspace window with an empty project.

mod app;
mod captions_panel;
mod credits;
mod frame_grid;
mod fx_panel;
mod icons;
mod loudness_panel;
mod precision_slider;
mod shortcuts;
mod splash;
mod waveform_paint;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("logs")))
        .unwrap_or_else(|| "logs".into());
    neoveg_common::logging::init(&log_dir);
    tracing::info!("[INFO] Neoveg starting (pure Rust workspace)");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Neoveg")
            .with_inner_size([1440.0, 860.0])
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
