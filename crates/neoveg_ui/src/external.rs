//! Things the welcome screen sends to the operating system: a web page in the browser and a folder
//! in the file manager. Neither waits for the program it starts.

use std::path::Path;
use std::process::{Command, Stdio};

/// The project on the web: the guide, the releases and the history all live under it.
pub const REPOSITORY: &str = "https://github.com/blackghoust9/neoveg";

/// The places of the *Learn* list of the welcome screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// How to use the program: the guide on the front page of the project.
    HowToUse,
    /// The published versions.
    Updates,
    /// What changed lately: the history of the project.
    WhatsNew,
}

impl Link {
    pub fn url(self) -> String {
        match self {
            Link::HowToUse => format!("{REPOSITORY}#readme"),
            Link::Updates => format!("{REPOSITORY}/releases"),
            Link::WhatsNew => format!("{REPOSITORY}/commits"),
        }
    }
}

/// Opens `target` (a web address or a folder) with what the system uses for it.
fn open_with_system(target: &str) -> Result<(), String> {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", target]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(target);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(target);
        c
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Opens a web address in the browser.
pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("endereço que não é da web: {url}"));
    }
    open_with_system(url)
}

/// Opens a folder in the file manager.
pub fn open_folder(folder: &Path) -> Result<(), String> {
    if !folder.is_dir() {
        return Err(format!("a pasta não existe: {}", folder.display()));
    }
    open_with_system(&folder.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_link_points_into_the_project() {
        for link in [Link::HowToUse, Link::Updates, Link::WhatsNew] {
            assert!(link.url().starts_with(REPOSITORY), "{}", link.url());
        }
        assert_ne!(Link::Updates.url(), Link::WhatsNew.url());
    }

    #[test]
    fn only_web_addresses_and_real_folders_are_opened() {
        assert!(open_url("file:///etc/passwd").is_err());
        assert!(open_url("calc.exe").is_err());
        assert!(open_folder(Path::new("/pasta/que/nao/existe/neoveg")).is_err());
    }
}
