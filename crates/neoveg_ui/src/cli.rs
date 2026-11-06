//! The command line: `neoveg [--open <projeto>] [--render <projeto> <saida>] [--screenshots <pasta>]
//! [--run-script <arquivo>] [--help] [--version]`.
//!
//! The parser is pure (it gets the arguments and returns what to do), so it is tested without a
//! window. A bare path with a project extension counts as `--open` (what the shell passes when
//! the user double-clicks a project).

use std::path::PathBuf;

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cli {
    /// Start the editor (opening `project` in a tab when given).
    Run { open: Option<PathBuf> },
    /// Render `project` to `output` without a window and exit.
    Render { project: PathBuf, output: PathBuf },
    /// Render the interface offscreen to PNG files in `dir` and exit.
    Screenshots { dir: PathBuf },
    /// Run a script file without a window and exit.
    RunScript { script: PathBuf },
    Help,
    Version,
}

/// The text of `--help`.
pub const HELP: &str = "Uso: neoveg [opções] [projeto]\n\
  --open <projeto>             abre o projeto numa aba ao iniciar\n\
  --render <projeto> <saida>   renderiza o projeto sem abrir a janela e sai\n\
  --run-script <arquivo>       executa um script sem abrir a janela e sai\n\
  --screenshots [pasta]        gera as capturas de tela da interface e sai\n\
  --help, -h                   mostra esta ajuda\n\
  --version, -V                mostra a versão";

/// Whether `text` looks like a project file (`.neoveg` or `.nvproj`).
fn is_project_path(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.ends_with(".neoveg") || lower.ends_with(".nvproj")
}

/// Reads the arguments (without the program name). An error is a message for the user.
pub fn parse<I, S>(args: I) -> Result<Cli, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let mut open: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(Cli::Help),
            "--version" | "-V" => return Ok(Cli::Version),
            "--open" => {
                let path = args
                    .next()
                    .ok_or_else(|| "--open precisa do caminho de um projeto".to_string())?;
                open = Some(path.into());
            }
            "--render" => {
                let project = args
                    .next()
                    .ok_or_else(|| "--render precisa do projeto e do arquivo de saída".to_string())?;
                let output = args
                    .next()
                    .ok_or_else(|| "--render precisa do arquivo de saída".to_string())?;
                return Ok(Cli::Render {
                    project: project.into(),
                    output: output.into(),
                });
            }
            "--run-script" => {
                let script = args
                    .next()
                    .ok_or_else(|| "--run-script precisa do caminho do script".to_string())?;
                return Ok(Cli::RunScript {
                    script: script.into(),
                });
            }
            "--screenshots" => {
                let dir = args.next().unwrap_or_else(|| "docs/screenshots".into());
                return Ok(Cli::Screenshots { dir: dir.into() });
            }
            other if other.starts_with('-') => {
                return Err(format!("opção desconhecida: {other}"));
            }
            other if is_project_path(other) && open.is_none() => open = Some(other.into()),
            other => return Err(format!("argumento inesperado: {other}")),
        }
    }
    Ok(Cli::Run { open })
}

/// The project the command line asked to open, kept for the window that starts later.
static STARTUP_OPEN: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Remembers the project to open when the editor starts.
pub fn set_startup_open(path: PathBuf) {
    let _ = STARTUP_OPEN.set(path);
}

/// The project to open when the editor starts, once.
pub fn startup_open() -> Option<PathBuf> {
    STARTUP_OPEN.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Cli, String> {
        parse(args.iter().copied())
    }

    #[test]
    fn no_arguments_starts_the_editor() {
        assert_eq!(p(&[]), Ok(Cli::Run { open: None }));
    }

    #[test]
    fn open_takes_a_project_and_a_bare_project_path_counts_as_open() {
        let want = Ok(Cli::Run {
            open: Some("a.neoveg".into()),
        });
        assert_eq!(p(&["--open", "a.neoveg"]), want);
        assert_eq!(p(&["a.neoveg"]), want);
        assert!(p(&["--open"]).is_err());
    }

    #[test]
    fn render_needs_project_and_output() {
        assert_eq!(
            p(&["--render", "a.neoveg", "out.mp4"]),
            Ok(Cli::Render {
                project: "a.neoveg".into(),
                output: "out.mp4".into()
            })
        );
        assert!(p(&["--render", "a.neoveg"]).is_err());
        assert!(p(&["--render"]).is_err());
    }

    #[test]
    fn script_screenshots_help_and_version() {
        assert_eq!(
            p(&["--run-script", "x.rhai"]),
            Ok(Cli::RunScript {
                script: "x.rhai".into()
            })
        );
        assert_eq!(
            p(&["--screenshots", "out"]),
            Ok(Cli::Screenshots { dir: "out".into() })
        );
        assert_eq!(
            p(&["--screenshots"]),
            Ok(Cli::Screenshots {
                dir: "docs/screenshots".into()
            })
        );
        assert_eq!(p(&["-h"]), Ok(Cli::Help));
        assert_eq!(p(&["--version"]), Ok(Cli::Version));
    }

    #[test]
    fn unknown_things_are_refused() {
        assert!(p(&["--nada"]).unwrap_err().contains("--nada"));
        assert!(p(&["lixo.txt"]).is_err());
    }
}

/// The render format the extension of the output file asks for (`None`: not one we write).
pub fn format_for_output(output: &std::path::Path) -> Option<neoveg_video::render::RenderFormat> {
    use neoveg_video::render::RenderFormat as F;
    match output.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "mp4" => Some(F::Mp4H264),
        "mov" => Some(F::MovProRes),
        "webm" => Some(F::WebmVp9),
        "wav" => Some(F::WavPcm16),
        _ => None,
    }
}

#[cfg(test)]
mod format_tests {
    use super::*;
    use neoveg_video::render::RenderFormat as F;
    use std::path::Path;

    #[test]
    fn the_extension_picks_the_format() {
        assert_eq!(format_for_output(Path::new("a/b.MP4")), Some(F::Mp4H264));
        assert_eq!(format_for_output(Path::new("b.wav")), Some(F::WavPcm16));
        assert_eq!(format_for_output(Path::new("b.webm")), Some(F::WebmVp9));
        assert_eq!(format_for_output(Path::new("b.mov")), Some(F::MovProRes));
        assert_eq!(format_for_output(Path::new("b.xyz")), None);
        assert_eq!(format_for_output(Path::new("b")), None);
    }
}
