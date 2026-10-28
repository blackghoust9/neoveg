//! Built-in Log Viewer window (`Exibir → Visualizador de Logs`): inspects `logs/neoveg.log` and
//! `logs/crash.log`, filters by level and search query, and provides a `Copiar Logs` button that
//! displays `Copiado!` for 2 seconds after copying.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};

const MAX_READ_BYTES: u64 = 256 * 1024;
const COPIED_FEEDBACK: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevelFilter {
    #[default]
    All,
    Info,
    Warn,
    Error,
}

impl LogLevelFilter {
    pub const ALL: [LogLevelFilter; 4] = [Self::All, Self::Info, Self::Warn, Self::Error];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Todos",
            Self::Info => "INFO+",
            Self::Warn => "WARN+",
            Self::Error => "ERROR / FATAL",
        }
    }

    pub fn matches_line(self, line: &str) -> bool {
        match self {
            Self::All | Self::Info => true,
            Self::Warn => {
                let u = line.to_ascii_uppercase();
                u.contains("WARN") || u.contains("ERROR") || u.contains("FATAL")
            }
            Self::Error => {
                let u = line.to_ascii_uppercase();
                u.contains("ERROR") || u.contains("FATAL")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogSourceFile {
    #[default]
    Main,
    Crash,
}

impl LogSourceFile {
    pub fn filename(self) -> &'static str {
        match self {
            Self::Main => "neoveg.log",
            Self::Crash => "crash.log",
        }
    }

    pub fn label(self) -> &'static str {
        self.filename()
    }
}

pub struct LogViewerState {
    pub open: bool,
    pub source: LogSourceFile,
    pub level: LogLevelFilter,
    pub search: String,
    pub auto_refresh: bool,
    cached_text: String,
    last_refresh: Option<Instant>,
    copied_until: Option<Instant>,
}

impl Default for LogViewerState {
    fn default() -> Self {
        Self {
            open: false,
            source: LogSourceFile::Main,
            level: LogLevelFilter::All,
            search: String::new(),
            auto_refresh: true,
            cached_text: String::new(),
            last_refresh: None,
            copied_until: None,
        }
    }
}

/// Reads at most the trailing `MAX_READ_BYTES` of `path` on a UTF-8 line boundary so large logs
/// never stall the UI.
pub fn read_log_tail(path: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut buf = Vec::new();
    if len > MAX_READ_BYTES {
        let _ = file.seek(SeekFrom::End(-(MAX_READ_BYTES as i64)));
        let _ = file.read_to_end(&mut buf);
        let text = String::from_utf8_lossy(&buf);
        if let Some(nl) = text.find('\n') {
            return text[nl + 1..].to_string();
        }
        return text.into_owned();
    }
    let _ = file.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

/// Filters log lines by `level` and case-insensitive substring `query`.
pub fn filter_log_lines<'a>(text: &'a str, level: LogLevelFilter, query: &str) -> Vec<&'a str> {
    let q = query.trim().to_lowercase();
    text.lines()
        .filter(|line| {
            level.matches_line(line) && (q.is_empty() || line.to_lowercase().contains(&q))
        })
        .collect()
}

pub fn default_log_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("logs")))
        .unwrap_or_else(|| PathBuf::from("logs"))
}

impl LogViewerState {
    pub fn refresh_now(&mut self, log_dir: &Path) {
        let path = log_dir.join(self.source.filename());
        self.cached_text = read_log_tail(&path);
        self.last_refresh = Some(Instant::now());
    }

    pub fn show(&mut self, ctx: &egui::Context, log_dir: &Path) {
        if !self.open {
            return;
        }
        let now = Instant::now();
        let needs_refresh = self.last_refresh.is_none_or(|t| {
            self.auto_refresh && now.saturating_duration_since(t) >= Duration::from_millis(1200)
        });
        if needs_refresh {
            self.refresh_now(log_dir);
        }

        let mut is_open = self.open;
        let mut force_refresh = false;
        crate::dialogs::tool_window(ctx, "Visualizador de Logs")
            .open(&mut is_open)
            .default_size([760.0, 440.0])
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for src in [LogSourceFile::Main, LogSourceFile::Crash] {
                        if ui
                            .selectable_label(self.source == src, src.label())
                            .clicked()
                        {
                            self.source = src;
                            force_refresh = true;
                        }
                    }
                    ui.separator();
                    egui::ComboBox::from_id_salt("log_viewer_level")
                        .selected_text(self.level.label())
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for candidate in LogLevelFilter::ALL {
                                ui.selectable_value(&mut self.level, candidate, candidate.label());
                            }
                        });
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Filtrar texto…")
                            .desired_width(180.0),
                    );
                    ui.checkbox(&mut self.auto_refresh, "Auto");
                    if ui.button("Atualizar").clicked() {
                        force_refresh = true;
                    }

                    let lines = filter_log_lines(&self.cached_text, self.level, &self.search);
                    let showing_copied = self.copied_until.is_some_and(|until| now < until);
                    if showing_copied {
                        ctx.request_repaint_after(Duration::from_millis(100));
                    } else {
                        self.copied_until = None;
                    }
                    let copy_label = if showing_copied {
                        "Copiado!"
                    } else {
                        "Copiar Logs"
                    };
                    if ui.button(copy_label).clicked() {
                        let joined = lines.join("\n");
                        ctx.copy_text(joined);
                        self.copied_until = Some(Instant::now() + COPIED_FEEDBACK);
                        ctx.request_repaint_after(COPIED_FEEDBACK);
                    }
                });

                if force_refresh {
                    self.refresh_now(log_dir);
                }

                ui.separator();
                let path = log_dir.join(self.source.filename());
                ui.weak(format!("Arquivo: {}", path.display()));
                ui.add_space(4.0);

                let lines = filter_log_lines(&self.cached_text, self.level, &self.search);
                if lines.is_empty() {
                    ui.weak("Nenhuma entrada de log encontrada para o filtro atual.");
                } else {
                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in lines {
                                let upper = line.to_ascii_uppercase();
                                let color = if upper.contains("FATAL") || upper.contains("ERROR") {
                                    Color32::from_rgb(242, 100, 92)
                                } else if upper.contains("WARN") {
                                    Color32::from_rgb(235, 186, 82)
                                } else {
                                    ui.visuals().text_color()
                                };
                                ui.label(RichText::new(line).monospace().size(11.5).color(color));
                            }
                        });
                }
            });
        self.open = is_open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matches_severity_and_case_insensitive_search() {
        let sample = "\
2026-10-27 INFO [INFO] Neoveg starting\n\
2026-10-27 WARN [WARN] LUT missing: warm.cube\n\
2026-10-27 ERROR [ERROR] audio device disconnected\n\
2026-10-27 ERROR [FATAL] panic in worker\n";

        assert_eq!(filter_log_lines(sample, LogLevelFilter::All, "").len(), 4);
        assert_eq!(filter_log_lines(sample, LogLevelFilter::Warn, "").len(), 3);
        assert_eq!(filter_log_lines(sample, LogLevelFilter::Error, "").len(), 2);
        let cube = filter_log_lines(sample, LogLevelFilter::All, "WARM.CUBE");
        assert_eq!(cube.len(), 1);
        assert!(cube[0].contains("warm.cube"));
    }

    #[test]
    fn read_log_tail_handles_missing_and_existing_files() {
        assert_eq!(read_log_tail(Path::new("/nonexistent/neoveg.log")), "");
        let p = std::env::temp_dir().join(format!("neoveg_log_tail_{}.log", std::process::id()));
        std::fs::write(&p, "[INFO] line 1\n[WARN] line 2\n").unwrap();
        let text = read_log_tail(&p);
        assert!(text.contains("[INFO] line 1") && text.contains("[WARN] line 2"));
        let _ = std::fs::remove_file(&p);
    }
}
