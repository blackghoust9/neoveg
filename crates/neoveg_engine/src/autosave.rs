//! Autosave files (`recovery_<session>.neoveg`) and crash recovery.
//!
//! A running session periodically writes its own recovery file and deletes it on a clean exit or
//! after a successful save. Whatever is left in the folder at the next start belongs to a session
//! that crashed (or was killed) and is offered for recovery. Only one Neoveg instance per user is
//! assumed: files written by a second running instance would look like crash leftovers.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use neoveg_common::project::ProjectInfo;

use crate::project_file::{read_recovery_meta, save_with_recovery, RecoveryMeta};
use crate::timeline::Timeline;

const PREFIX: &str = "recovery_";

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryEntry {
    pub path: PathBuf,
    pub meta: RecoveryMeta,
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Identifier of this run (process id + start time), safe to use in a file name.
pub fn new_session_id() -> String {
    format!("{}_{}", std::process::id(), unix_now())
}

pub fn recovery_path(dir: &Path, session_id: &str) -> PathBuf {
    dir.join(format!(
        "{PREFIX}{session_id}.{}",
        crate::project_file::EXTENSION
    ))
}

/// Writes (atomically replacing) the recovery file of `session_id`.
pub fn write_recovery(
    dir: &Path,
    session_id: &str,
    info: &ProjectInfo,
    timeline: &Timeline,
    original_path: Option<&Path>,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = recovery_path(dir, session_id);
    let meta = RecoveryMeta {
        original_path: original_path.map(|p| p.to_string_lossy().into_owned()),
        project_name: info.name.clone(),
        saved_unix: unix_now(),
    };
    save_with_recovery(&path, info, timeline, Some(&meta))?;
    Ok(path)
}

/// Recovery files in `dir`, newest first. Files that are not readable autosaves are ignored.
pub fn list_recoveries(dir: &Path) -> Vec<RecoveryEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<RecoveryEntry> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(PREFIX))
                && p.extension()
                    .is_some_and(|e| e == crate::project_file::EXTENSION)
        })
        .filter_map(|path| {
            let meta = read_recovery_meta(&path)?;
            Some(RecoveryEntry { path, meta })
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.meta.saved_unix));
    entries
}

/// Deletes a recovery file; a missing file is not an error.
pub fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Deletes recovery files older than `max_age`; returns how many were removed.
pub fn prune_older_than(dir: &Path, max_age: Duration) -> usize {
    let cutoff = unix_now().saturating_sub(max_age.as_secs());
    list_recoveries(dir)
        .into_iter()
        .filter(|e| e.meta.saved_unix < cutoff)
        .map(|e| remove(&e.path))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{EditCommand, History};
    use crate::project_file;
    use crate::timeline::TrackKind;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("neoveg_auto_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn project() -> (ProjectInfo, Timeline) {
        let mut tl = Timeline::new();
        let track = tl.new_track(TrackKind::Audio);
        History::new().execute(&mut tl, EditCommand::AddTrack { track });
        let mut info = ProjectInfo::create_empty();
        info.name = "Em edicao".into();
        (info, tl)
    }

    #[test]
    fn recovery_file_round_trips_and_keeps_its_metadata() {
        let dir = temp_dir("roundtrip");
        let (info, tl) = project();
        let original = Path::new("C:/projetos/filme.neoveg");
        let path = write_recovery(&dir, "42_1", &info, &tl, Some(original)).unwrap();
        assert!(path.ends_with("recovery_42_1.neoveg"));

        let (loaded_info, loaded_tl) = project_file::load(&path).unwrap();
        assert_eq!(loaded_info, info);
        assert_eq!(loaded_tl, tl);

        let found = list_recoveries(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].meta.project_name, "Em edicao");
        assert_eq!(
            found[0].meta.original_path.as_deref(),
            Some("C:/projetos/filme.neoveg")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rewriting_replaces_the_same_file() {
        let dir = temp_dir("replace");
        let (info, tl) = project();
        write_recovery(&dir, "7_1", &info, &tl, None).unwrap();
        write_recovery(&dir, "7_1", &info, &tl, None).unwrap();
        assert_eq!(list_recoveries(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn regular_projects_and_junk_are_not_listed() {
        let dir = temp_dir("filter");
        let (info, tl) = project();
        std::fs::create_dir_all(&dir).unwrap();
        project_file::save(&dir.join("recovery_manual.neoveg"), &info, &tl).unwrap();
        std::fs::write(dir.join("recovery_bad.neoveg"), b"junk").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        assert!(list_recoveries(&dir).is_empty());
        assert!(list_recoveries(&dir.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn remove_deletes_and_tolerates_missing_files() {
        let dir = temp_dir("remove");
        let (info, tl) = project();
        let path = write_recovery(&dir, "1_1", &info, &tl, None).unwrap();
        remove(&path);
        remove(&path);
        assert!(list_recoveries(&dir).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_removes_only_old_files() {
        let dir = temp_dir("prune");
        let (info, tl) = project();
        write_recovery(&dir, "new_1", &info, &tl, None).unwrap();
        // an autosave written long ago
        let old = recovery_path(&dir, "old_1");
        let meta = RecoveryMeta {
            original_path: None,
            project_name: "antigo".into(),
            saved_unix: unix_now() - 40 * 86_400,
        };
        save_with_recovery(&old, &info, &tl, Some(&meta)).unwrap();

        assert_eq!(prune_older_than(&dir, Duration::from_secs(30 * 86_400)), 1);
        let left = list_recoveries(&dir);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].meta.project_name, "Em edicao");
        let _ = std::fs::remove_dir_all(dir);
    }
}
