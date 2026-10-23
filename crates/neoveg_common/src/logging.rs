//! Central logging: `logs/neoveg.log` in the application root, plus a panic hook for `logs/crash.log`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

/// Installs a `tracing` subscriber writing to `<log_dir>/neoveg.log` and a panic hook writing to
/// `<log_dir>/crash.log`. Falls back to stderr when the log file cannot be opened.
pub fn init(log_dir: &Path) {
    let _ = fs::create_dir_all(log_dir);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    match OpenOptions::new().create(true).append(true).open(log_dir.join("neoveg.log")) {
        Ok(file) => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(Mutex::new(file))
                .try_init();
        }
        Err(_) => {
            let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
        }
    }

    let crash_path = log_dir.join("crash.log");
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("[FATAL] {info}");
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&crash_path) {
            let _ = writeln!(f, "[FATAL] {info}");
        }
    }));
}
