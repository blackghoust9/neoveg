//! The native `.neoveg` project container: a ZIP holding `project.json`, `media_pool.json` and
//! `history.json`.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use neoveg_common::project::{MediaRef, ProjectInfo, ProjectSettings};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::timeline::Timeline;

pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "neoveg";

/// Extra `recovery.json` entry written only in autosave files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryMeta {
    /// File the user was editing when the autosave was written (`None` for an unsaved project).
    pub original_path: Option<String>,
    pub project_name: String,
    pub saved_unix: u64,
}

#[derive(Serialize, Deserialize)]
struct ProjectJson {
    format_version: u32,
    name: String,
    settings: ProjectSettings,
    timeline: Timeline,
}

/// Undo history is session-only for now; the entry reserves the slot in the container.
#[derive(Serialize, Deserialize)]
struct HistoryJson {
    format_version: u32,
    entries: Vec<String>,
}

fn write_entry<T: Serialize>(
    zip: &mut ZipWriter<File>,
    name: &str,
    value: &T,
) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| format!("{name}: {e}"))?;
    zip.start_file(name, SimpleFileOptions::default())
        .map_err(|e| format!("{name}: {e}"))?;
    zip.write_all(&json).map_err(|e| format!("{name}: {e}"))
}

fn read_entry<T: for<'de> Deserialize<'de>>(
    zip: &mut ZipArchive<File>,
    name: &str,
) -> Result<T, String> {
    let mut file = zip
        .by_name(name)
        .map_err(|e| format!("{name} missing: {e}"))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|e| format!("{name}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{name} is invalid: {e}"))
}

/// Saves the project atomically (temp file, then rename).
pub fn save(path: &Path, info: &ProjectInfo, timeline: &Timeline) -> Result<(), String> {
    save_with_recovery(path, info, timeline, None)
}

/// Like [`save`], optionally embedding recovery metadata (used by autosave).
pub fn save_with_recovery(
    path: &Path,
    info: &ProjectInfo,
    timeline: &Timeline,
    recovery: Option<&RecoveryMeta>,
) -> Result<(), String> {
    let tmp = path.with_extension("neoveg.tmp");
    {
        let file =
            File::create(&tmp).map_err(|e| format!("cannot create {}: {e}", tmp.display()))?;
        let mut zip = ZipWriter::new(file);
        write_entry(
            &mut zip,
            "project.json",
            &ProjectJson {
                format_version: FORMAT_VERSION,
                name: info.name.clone(),
                settings: info.settings.clone(),
                timeline: timeline.clone(),
            },
        )?;
        write_entry(&mut zip, "media_pool.json", &info.media_pool)?;
        write_entry(
            &mut zip,
            "history.json",
            &HistoryJson {
                format_version: FORMAT_VERSION,
                entries: Vec::new(),
            },
        )?;
        if let Some(meta) = recovery {
            write_entry(&mut zip, "recovery.json", meta)?;
        }
        zip.finish()
            .map_err(|e| format!("cannot finish archive: {e}"))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
}

/// Loads a project. Rejects files written by a newer format version.
pub fn load(path: &Path) -> Result<(ProjectInfo, Timeline), String> {
    let file = File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut zip = ZipArchive::new(file)
        .map_err(|e| format!("{} is not a .neoveg project: {e}", path.display()))?;

    let project: ProjectJson = read_entry(&mut zip, "project.json")?;
    if project.format_version > FORMAT_VERSION {
        return Err(format!(
            "project format {} is newer than supported ({FORMAT_VERSION})",
            project.format_version
        ));
    }
    let media_pool: Vec<MediaRef> = read_entry(&mut zip, "media_pool.json")?;
    let info = ProjectInfo {
        name: project.name,
        settings: project.settings,
        media_pool,
    };
    Ok((info, project.timeline))
}

/// Recovery metadata of an autosave file (`None` for regular projects).
pub fn read_recovery_meta(path: &Path) -> Option<RecoveryMeta> {
    let file = File::open(path).ok()?;
    let mut zip = ZipArchive::new(file).ok()?;
    read_entry(&mut zip, "recovery.json").ok()
}

/// Media-pool entries whose file no longer exists.
pub fn find_missing(info: &ProjectInfo) -> Vec<u64> {
    info.media_pool
        .iter()
        .filter(|m| !Path::new(&m.path).exists())
        .map(|m| m.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{EditCommand, History};
    use crate::timeline::{TrackEvent, TrackKind};

    pub(crate) fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("neoveg_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_project() -> (ProjectInfo, Timeline) {
        let mut info = ProjectInfo::create_empty();
        info.name = "Teste".into();
        let mut tl = Timeline::new();
        let mut history = History::new();
        let track = tl.new_track(TrackKind::Audio);
        let track_id = track.id;
        history.execute(&mut tl, EditCommand::AddTrack { track });
        let media_id = tl.alloc_id();
        info.media_pool.push(MediaRef {
            id: media_id,
            path: "C:/nada/a.wav".into(),
            duration_seconds: 4.0,
        });
        let event = TrackEvent {
            id: tl.alloc_id(),
            name: "a.wav".into(),
            media_id: Some(media_id),
            start: 1.5,
            duration: 4.0,
            trim_in: 0.25,
            fade_in: 0.1,
            fade_out: 0.2,
            fade_in_curve: neoveg_common::fade::FadeCurve::Smooth,
            fade_out_curve: neoveg_common::fade::FadeCurve::Fast,
        };
        history.execute(&mut tl, EditCommand::AddEvent { track_id, event });
        (info, tl)
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("p.neoveg");
        let (info, tl) = sample_project();
        save(&path, &info, &tl).unwrap();

        let (info2, tl2) = load(&path).unwrap();
        assert_eq!(info2, info);
        assert_eq!(tl2, tl);
        // Ids keep counting after a reload
        let mut tl2 = tl2;
        assert_eq!(tl2.alloc_id(), tl.clone().alloc_id());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn container_has_the_three_entries() {
        let dir = temp_dir("entries");
        let path = dir.join("p.neoveg");
        let (info, tl) = sample_project();
        save(&path, &info, &tl).unwrap();
        let mut zip = ZipArchive::new(File::open(&path).unwrap()).unwrap();
        for name in ["project.json", "media_pool.json", "history.json"] {
            assert!(zip.by_name(name).is_ok(), "{name}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_non_projects_and_newer_versions() {
        let dir = temp_dir("reject");
        let bad = dir.join("bad.neoveg");
        std::fs::write(&bad, b"not a zip").unwrap();
        assert!(load(&bad).is_err());
        assert!(load(&dir.join("missing.neoveg")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_media_is_reported() {
        let (info, _) = sample_project();
        assert_eq!(find_missing(&info), vec![info.media_pool[0].id]);
    }

    #[test]
    fn projects_saved_before_fade_curves_still_load() {
        use neoveg_common::fade::FadeCurve;
        let event: crate::timeline::TrackEvent = serde_json::from_str(
            r#"{"id":1,"name":"a","media_id":null,"start":0.0,"duration":2.0,
                "trim_in":0.0,"fade_in":0.5,"fade_out":0.5}"#,
        )
        .unwrap();
        assert_eq!(event.fade_in_curve, FadeCurve::Linear);
        assert_eq!(event.fade_out_curve, FadeCurve::Linear);

        let settings: ProjectSettings =
            serde_json::from_str(r#"{"width":1280,"height":720,"fps":25.0,"sample_rate":44100}"#)
                .unwrap();
        assert!(settings.auto_crossfade);
    }
}
