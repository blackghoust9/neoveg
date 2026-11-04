//! Central logging: `logs/neoveg.log` in the application root, plus a panic hook for `logs/crash.log`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

/// Log levels when `RUST_LOG` is not set: Neoveg and everything else at `info`, but the graphics
/// stack at `warn` (wgpu logs one `info` line per submission, about 60 a second, which buries the
/// program's own messages and grows the file by megabytes).
pub const DEFAULT_FILTER: &str = "info,wgpu_core=warn,wgpu_hal=warn,naga=warn";

/// Installs a `tracing` subscriber writing to `<log_dir>/neoveg.log` and a panic hook writing to
/// `<log_dir>/crash.log`. Falls back to stderr when the log file cannot be opened.
pub fn init(log_dir: &Path) {
    let _ = fs::create_dir_all(log_dir);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_FILTER));

    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("neoveg.log"))
    {
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
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_path)
        {
            let _ = writeln!(f, "[FATAL] {info}");
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_filter_parses_and_quiets_the_graphics_stack() {
        assert!(tracing_subscriber::EnvFilter::try_new(DEFAULT_FILTER).is_ok());
        for noisy in ["wgpu_core=warn", "wgpu_hal=warn", "naga=warn"] {
            assert!(DEFAULT_FILTER.contains(noisy), "{noisy}");
        }
        // the program's own messages stay at info
        assert!(DEFAULT_FILTER.starts_with("info,"));
    }
}
