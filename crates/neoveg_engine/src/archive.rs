//! *Arquivar projeto* and *Consolidar mídia*: copy a project and the media it uses into one folder.
//!
//! The plan is pure: it reads the timeline, finds the part of every media file that the events
//! use, adds a safety margin at both ends and decides the file name in the new folder. Running the
//! plan copies the files (a whole copy, or a cut of the used part through a callback the caller
//! provides, so this crate needs no FFmpeg) and writes the project with its media pointing at the
//! copies. A cut that fails falls back to a whole copy, so the archive never loses media.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use neoveg_common::project::ProjectInfo;

use crate::timeline::Timeline;

/// Name of the sub-folder of the archive that holds the media.
pub const MEDIA_FOLDER: &str = "Midia";

/// What to keep of each file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArchiveOptions {
    /// Cut every file down to the part the events use (Consolidar) instead of copying it whole.
    pub only_used: bool,
    /// Seconds kept before the first and after the last used moment, when `only_used` is set.
    pub margin: f64,
}

impl Default for ArchiveOptions {
    fn default() -> Self {
        Self {
            only_used: false,
            margin: 2.0,
        }
    }
}

/// One media file of the plan.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchiveItem {
    pub media_id: u64,
    pub source: PathBuf,
    /// File name inside [`MEDIA_FOLDER`].
    pub dest_name: String,
    /// The part of the source to keep, in source seconds. `None`: the whole file.
    pub segment: Option<(f64, f64)>,
    /// `true` when the source file is not there.
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArchivePlan {
    pub items: Vec<ArchiveItem>,
}

/// Source span, in seconds, that an event plays: from `trim_in` for `duration` of timeline time.
fn used_span(event: &crate::timeline::TrackEvent, media_len: f64) -> (f64, f64) {
    let start = event.trim_in.max(0.0);
    if event.looping {
        return (start, media_len);
    }
    let rate = if event.playback_rate.is_finite() && event.playback_rate > 0.0 {
        event.playback_rate
    } else {
        1.0
    };
    (
        start,
        (start + event.duration * rate).min(media_len.max(start)),
    )
}

/// Builds the plan for the media the timeline uses. Media in the pool that no event uses is left
/// out. `exists` tells whether a source file is there (a parameter so the tests need no files).
pub fn plan(
    info: &ProjectInfo,
    timeline: &Timeline,
    options: ArchiveOptions,
    exists: impl Fn(&Path) -> bool,
) -> ArchivePlan {
    // Smallest start and largest end of what the events use, per media.
    let mut spans: BTreeMap<u64, (f64, f64)> = BTreeMap::new();
    for track in &timeline.tracks {
        for event in track.events.iter().filter(|e| !e.adjustment) {
            let Some(id) = event.media_id else { continue };
            let Some(media) = info.media_pool.iter().find(|m| m.id == id) else {
                continue;
            };
            let (a, b) = used_span(event, media.duration_seconds);
            let entry = spans.entry(id).or_insert((a, b));
            entry.0 = entry.0.min(a);
            entry.1 = entry.1.max(b);
        }
    }
    let mut taken: HashSet<String> = HashSet::new();
    let mut items = Vec::new();
    for media in &info.media_pool {
        let Some(&(a, b)) = spans.get(&media.id) else {
            continue;
        };
        let source = PathBuf::from(&media.path);
        let base = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("midia_{}", media.id));
        let dest_name = unique_name(&base, &mut taken);
        let segment = if options.only_used && !media.subproject && media.duration_seconds > 0.0 {
            let margin = options.margin.max(0.0);
            let start = (a - margin).max(0.0);
            let end = (b + margin).min(media.duration_seconds);
            // A span that covers (nearly) the whole file is a whole copy.
            (end - start < media.duration_seconds - 1e-3 && end > start).then_some((start, end))
        } else {
            None
        };
        items.push(ArchiveItem {
            media_id: media.id,
            missing: !exists(&source),
            source,
            dest_name,
            segment,
        });
    }
    ArchivePlan { items }
}

/// `name`, or `name_2.ext`, `name_3.ext`... when the name is already taken.
fn unique_name(name: &str, taken: &mut HashSet<String>) -> String {
    let key = |s: &str| s.to_lowercase();
    if taken.insert(key(name)) {
        return name.to_string();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| format!("{stem}_{n}{ext}"))
        .find(|candidate| taken.insert(key(candidate)))
        .expect("an unbounded range always yields a free name")
}

/// What running the plan did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArchiveReport {
    /// Media copied whole.
    pub copied: Vec<u64>,
    /// Media cut to the part used: id and where the kept part starts in the source.
    pub cut: Vec<(u64, f64)>,
    /// Media whose source file was missing (left pointing at the old path).
    pub missing: Vec<u64>,
    /// Bytes written to the media folder.
    pub bytes: u64,
}

/// Callback used to cut a segment out of a source file during archiving.
pub type ArchiveCutter<'a> = dyn FnMut(&Path, &Path, f64, f64) -> Result<(), String> + 'a;

/// Copies the media of `plan` into `dest/Midia`. `cutter(source, dest, start, end)` writes the
/// part of `source` between the two times; when it fails the whole file is copied instead.
pub fn run(
    plan: &ArchivePlan,
    dest: &Path,
    cutter: &mut ArchiveCutter,
) -> Result<ArchiveReport, String> {
    let media_dir = dest.join(MEDIA_FOLDER);
    std::fs::create_dir_all(&media_dir)
        .map_err(|e| format!("não foi possível criar {}: {e}", media_dir.display()))?;
    let mut report = ArchiveReport::default();
    for item in &plan.items {
        if item.missing {
            report.missing.push(item.media_id);
            continue;
        }
        let target = media_dir.join(&item.dest_name);
        let mut cut_start = None;
        if let Some((a, b)) = item.segment {
            if cutter(&item.source, &target, a, b).is_ok() && target.is_file() {
                cut_start = Some(a);
            }
        }
        match cut_start {
            Some(a) => report.cut.push((item.media_id, a)),
            None => {
                std::fs::copy(&item.source, &target).map_err(|e| {
                    format!("não foi possível copiar {}: {e}", item.source.display())
                })?;
                report.copied.push(item.media_id);
            }
        }
        report.bytes += std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
    }
    Ok(report)
}

/// The project as it must be saved in the archive: media point at the copies (relative to the
/// project file, `Midia/<name>`) and the events of a cut file start earlier by the cut offset.
pub fn rewrite_project(
    info: &ProjectInfo,
    timeline: &Timeline,
    plan: &ArchivePlan,
    report: &ArchiveReport,
) -> (ProjectInfo, Timeline) {
    let mut info = info.clone();
    let mut timeline = timeline.clone();
    for item in &plan.items {
        if report.missing.contains(&item.media_id) {
            continue;
        }
        let offset = report
            .cut
            .iter()
            .find(|(id, _)| *id == item.media_id)
            .map(|&(_, a)| a);
        if let Some(media) = info.media_pool.iter_mut().find(|m| m.id == item.media_id) {
            media.path = format!("{MEDIA_FOLDER}/{}", item.dest_name);
            if let (Some(a), Some((_, b))) = (offset, item.segment) {
                media.duration_seconds = b - a;
            }
        }
        if let Some(a) = offset {
            for track in &mut timeline.tracks {
                for event in track
                    .events
                    .iter_mut()
                    .filter(|e| e.media_id == Some(item.media_id))
                {
                    event.trim_in = (event.trim_in - a).max(0.0);
                }
            }
        }
    }
    (info, timeline)
}

/// Archives the project into the folder `dest`: copies the media, writes `<name>.neoveg` with the
/// new paths, and returns what was done.
pub fn archive_project(
    info: &ProjectInfo,
    timeline: &Timeline,
    dest: &Path,
    options: ArchiveOptions,
    cutter: &mut ArchiveCutter,
) -> Result<ArchiveReport, String> {
    let plan = plan(info, timeline, options, |p| p.is_file());
    let report = run(&plan, dest, cutter)?;
    let (info2, timeline2) = rewrite_project(info, timeline, &plan, &report);
    let file = dest.join(format!(
        "{}.{}",
        sanitize_file_name(&info.name),
        crate::project_file::EXTENSION
    ));
    crate::project_file::save(&file, &info2, &timeline2)?;
    Ok(report)
}

/// A project name that is safe as a file name.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "Projeto".into()
    } else {
        cleaned
    }
}

/// Text written next to a rendered file so it can be traced back to its project: the project name,
/// where the project file is, and the part of the timeline rendered.
pub fn render_reference(
    project_name: &str,
    project_path: Option<&Path>,
    range: (f64, f64),
) -> String {
    let path = project_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(projeto ainda não salvo)".into());
    format!(
        "{{\"projeto\":{},\"arquivo_do_projeto\":{},\"inicio_s\":{:.3},\"fim_s\":{:.3}}}",
        serde_json::Value::String(project_name.to_string()),
        serde_json::Value::String(path),
        range.0,
        range.1
    )
}

/// Where the reference of the rendered file `output` is written: `<output>.projeto.json`.
pub fn render_reference_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_os_string();
    name.push(".projeto.json");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{TrackEvent, TrackKind};
    use neoveg_common::project::MediaRef;

    fn media(id: u64, path: &str, len: f64) -> MediaRef {
        MediaRef {
            id,
            path: path.into(),
            duration_seconds: len,
            subproject: false,
            format: None,
        }
    }

    fn project(paths: &[(&str, f64)]) -> (ProjectInfo, Timeline) {
        let mut info = ProjectInfo::create_empty();
        info.name = "Meu: projeto".into();
        let mut tl = Timeline::new();
        let mut track = tl.new_track(TrackKind::Video);
        for (i, (path, len)) in paths.iter().enumerate() {
            let id = i as u64 + 1;
            info.media_pool.push(media(id, path, *len));
            let eid = tl.alloc_id();
            track.events.push(TrackEvent {
                id: eid,
                media_id: Some(id),
                start: i as f64 * 10.0,
                duration: 5.0,
                trim_in: 20.0,
                ..TrackEvent::default()
            });
        }
        tl.tracks.push(track);
        (info, tl)
    }

    #[test]
    fn the_plan_keeps_the_used_part_with_a_margin() {
        let (info, tl) = project(&[("C:/m/a.mp4", 60.0)]);
        let opts = ArchiveOptions {
            only_used: true,
            margin: 3.0,
        };
        let p = plan(&info, &tl, opts, |_| true);
        assert_eq!(p.items[0].segment, Some((17.0, 28.0)));
        // the margin never goes past the ends of the file
        let opts = ArchiveOptions {
            only_used: true,
            margin: 100.0,
        };
        let p = plan(&info, &tl, opts, |_| true);
        assert_eq!(p.items[0].segment, None, "covers the whole file");
        let p = plan(&info, &tl, ArchiveOptions::default(), |_| true);
        assert_eq!(p.items[0].segment, None, "archive copies whole");
    }

    #[test]
    fn unused_media_is_left_out_and_equal_names_are_told_apart() {
        let (mut info, tl) = project(&[("C:/x/a.mp4", 60.0), ("C:/y/A.mp4", 60.0)]);
        info.media_pool.push(media(9, "C:/z/unused.mp4", 5.0));
        let p = plan(&info, &tl, ArchiveOptions::default(), |_| true);
        let names: Vec<_> = p.items.iter().map(|i| i.dest_name.as_str()).collect();
        assert_eq!(names, vec!["a.mp4", "A_2.mp4"]);
    }

    #[test]
    fn a_looping_event_keeps_everything_after_its_start() {
        let (info, mut tl) = project(&[("C:/m/a.mp4", 60.0)]);
        tl.tracks[0].events[0].looping = true;
        let opts = ArchiveOptions {
            only_used: true,
            margin: 0.0,
        };
        let p = plan(&info, &tl, opts, |_| true);
        assert_eq!(p.items[0].segment, Some((20.0, 60.0)));
    }

    #[test]
    fn running_the_plan_copies_files_and_rewrites_the_project() {
        let base = std::env::temp_dir().join(format!("neoveg_archive_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("src")).unwrap();
        let a = base.join("src").join("a.wav");
        let b = base.join("src").join("b.wav");
        std::fs::write(&a, vec![1u8; 100]).unwrap();
        std::fs::write(&b, vec![2u8; 50]).unwrap();
        let (info, tl) = project(&[(a.to_str().unwrap(), 60.0), (b.to_str().unwrap(), 60.0)]);
        let dest = base.join("out");
        let opts = ArchiveOptions {
            only_used: true,
            margin: 1.0,
        };
        // The cutter stands in for FFmpeg: it writes 10 bytes and fails for the second file.
        let mut cutter = |src: &Path, dst: &Path, _a: f64, _b: f64| -> Result<(), String> {
            if src.ends_with("b.wav") {
                return Err("sem ffmpeg".into());
            }
            std::fs::write(dst, [0u8; 10]).map_err(|e| e.to_string())
        };
        let report = archive_project(&info, &tl, &dest, opts, &mut cutter).unwrap();
        assert_eq!(report.cut, vec![(1, 19.0)]);
        assert_eq!(
            report.copied,
            vec![2],
            "a failed cut falls back to a whole copy"
        );
        assert_eq!(report.bytes, 60);
        assert_eq!(std::fs::read(dest.join("Midia/a.wav")).unwrap().len(), 10);
        assert_eq!(std::fs::read(dest.join("Midia/b.wav")).unwrap().len(), 50);

        let saved = dest.join("Meu_ projeto.neoveg");
        let (info2, tl2) = crate::project_file::load(&saved).unwrap();
        assert_eq!(info2.media_pool[0].path, "Midia/a.wav");
        assert_eq!(info2.media_pool[0].duration_seconds, 7.0);
        assert_eq!(tl2.tracks[0].events[0].trim_in, 1.0, "20 - 19");
        assert_eq!(tl2.tracks[0].events[1].trim_in, 20.0, "whole copy keeps it");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn missing_sources_are_reported_and_kept_as_they_were() {
        let (info, tl) = project(&[("C:/nao/existe.mp4", 60.0)]);
        let dest = std::env::temp_dir().join(format!("neoveg_archive_miss_{}", std::process::id()));
        let report = archive_project(
            &info,
            &tl,
            &dest,
            ArchiveOptions::default(),
            &mut |_, _, _, _| Ok(()),
        )
        .unwrap();
        assert_eq!(report.missing, vec![1]);
        let (info2, _) = crate::project_file::load(&dest.join("Meu_ projeto.neoveg")).unwrap();
        assert_eq!(info2.media_pool[0].path, "C:/nao/existe.mp4");
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn the_render_reference_names_project_and_range() {
        let text = render_reference("Filme \"X\"", Some(Path::new("C:/p/x.neoveg")), (1.0, 2.5));
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["projeto"], "Filme \"X\"");
        assert_eq!(v["fim_s"], 2.5);
        assert_eq!(
            render_reference_path(Path::new("out/a.mp4")),
            PathBuf::from("out/a.mp4.projeto.json")
        );
    }
}
