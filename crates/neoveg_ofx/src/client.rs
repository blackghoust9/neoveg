//! The program's side of the host: asking a **child process** what a plug-in library holds.
//!
//! The child (`neoveg_ofx_host`, see `describe.rs`) loads the library and calls the plug-ins; the
//! program only starts it, waits for it with a time limit and reads the answer from what it prints.
//! A plug-in that crashes, hangs, prints rubbish or exits takes the child with it and nothing else:
//! the program gets an error with the reason.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::report::{Answer, PluginReport};

/// How the child is run.
#[derive(Debug, Clone)]
pub struct Options {
    /// How long the child may take before it is killed.
    pub timeout: Duration,
    /// Variables of the environment to give the child on top of the program's.
    pub env: Vec<(String, String)>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
            env: Vec::new(),
        }
    }
}

/// The folder of a bundle that holds the library, by system.
fn bundle_folder() -> &'static str {
    if cfg!(target_os = "windows") {
        "Win64"
    } else if cfg!(target_os = "macos") {
        "MacOS"
    } else {
        "Linux-x86-64"
    }
}

/// The library to load for `path`: the file itself, or the `.ofx` file inside a `.ofx.bundle`.
pub fn library_of(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    if !path.is_dir() {
        return Err(format!("não existe: {}", path.display()));
    }
    let folder = path.join("Contents").join(bundle_folder());
    let mut found: Vec<PathBuf> = std::fs::read_dir(&folder)
        .map_err(|e| format!("o pacote não tem {}: {e}", folder.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ofx")))
        .collect();
    found.sort();
    found
        .into_iter()
        .next()
        .ok_or_else(|| format!("nenhum arquivo .ofx em {}", folder.display()))
}

/// The child program: the one that sits next to the running program.
pub fn default_host_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = format!("neoveg_ofx_host{}", std::env::consts::EXE_SUFFIX);
    exe.parent()
        .map(|dir| dir.join(&name))
        .filter(|path| path.is_file())
        .ok_or_else(|| format!("não achei {name} ao lado do programa"))
}

/// Reads a pipe to its end on a thread of its own, so a child that writes a lot never blocks.
fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Describes the plug-ins of `path` (a library or a bundle) in a child process, `host_exe` being the
/// child program.
pub fn describe(
    path: &Path,
    host_exe: &Path,
    options: &Options,
) -> Result<Vec<PluginReport>, String> {
    let library = library_of(path)?;
    let mut child = Command::new(host_exe)
        .arg("describe")
        .arg(&library)
        .envs(options.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("não foi possível iniciar {}: {e}", host_exe.display()))?;
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= options.timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "o plug-in não respondeu em {} s e o processo foi encerrado",
                    options.timeout.as_secs_f32()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(format!("falha ao esperar o processo do plug-in: {e}")),
        }
    };
    let output = stdout.and_then(|t| t.join().ok()).unwrap_or_default();
    let errors = stderr.and_then(|t| t.join().ok()).unwrap_or_default();

    match Answer::from_output(&output) {
        Ok(Answer::Plugins(plugins)) if status.success() => Ok(plugins),
        Ok(Answer::Error(message)) => Err(message),
        // It answered but did not end well, or did not answer at all: say how it ended
        _ => {
            let last = errors.lines().rev().find(|l| !l.trim().is_empty());
            Err(match last {
                Some(line) => format!("o processo do plug-in encerrou com {status} ({line})"),
                None => format!("o processo do plug-in encerrou com {status}"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neoveg_ofx_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_bundle_gives_the_library_inside_it_and_a_file_is_itself() {
        let root = temp("bundle");
        let bundle = root.join("Invert.ofx.bundle");
        let inside = bundle.join("Contents").join(bundle_folder());
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::write(inside.join("notes.txt"), b"x").unwrap();
        // No library yet
        assert!(library_of(&bundle)
            .unwrap_err()
            .contains("nenhum arquivo .ofx"));
        let library = inside.join("Invert.ofx");
        std::fs::write(&library, b"x").unwrap();
        assert_eq!(library_of(&bundle), Ok(library.clone()));
        assert_eq!(library_of(&library), Ok(library));
        assert!(library_of(&root.join("missing.ofx"))
            .unwrap_err()
            .contains("não existe"));
        let empty = root.join("Empty.ofx.bundle");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(library_of(&empty).unwrap_err().contains("o pacote não tem"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_child_program_that_does_not_exist_is_an_error_not_a_crash() {
        let root = temp("nohost");
        let library = root.join("x.ofx");
        std::fs::write(&library, b"x").unwrap();
        let error =
            describe(&library, &root.join("no_such_host"), &Options::default()).unwrap_err();
        assert!(error.contains("não foi possível iniciar"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }
}
