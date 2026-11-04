//! Missing-media relink.
//!
//! Step 1: exact file name in the project's own folder (sibling auto-relink) or in the search folders.
//! Step 2: fuzzy file-name match (case/separator-insensitive stem, or one stem containing the other).
//! Step 3 (metadata validation with duration/channel checks) is done by the caller on the candidates.

use std::path::{Path, PathBuf};

use neoveg_common::project::MediaRef;

fn normalize(stem: &str) -> String {
    stem.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(s, _)| s)
}

fn candidates(dirs: &[PathBuf]) -> Vec<PathBuf> {
    // The same folder can be listed twice (the project's and a media's); a file counts once
    let mut seen = std::collections::HashSet::new();
    dirs.iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|rd| rd.filter_map(Result::ok))
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

/// Step 1: exact (case-insensitive) file-name match.
pub fn find_exact(media: &MediaRef, dirs: &[PathBuf]) -> Option<PathBuf> {
    let wanted = file_name(&media.path).to_lowercase();
    candidates(dirs).into_iter().find(|p| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().to_lowercase() == wanted)
    })
}

/// Step 2: fuzzy match on the normalised stem, same extension required.
pub fn find_fuzzy(media: &MediaRef, dirs: &[PathBuf]) -> Option<PathBuf> {
    let name = file_name(&media.path);
    let wanted = normalize(stem(name));
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    if wanted.is_empty() {
        return None;
    }
    candidates(dirs).into_iter().find(|p| {
        let same_ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()) == ext;
        let cand = normalize(
            &p.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
        same_ext && !cand.is_empty() && (cand.contains(&wanted) || wanted.contains(&cand))
    })
}

/// Relinks every entry in `missing` it can find; returns how many were relinked.
pub fn relink_missing(pool: &mut [MediaRef], missing: &[u64], dirs: &[PathBuf]) -> usize {
    let mut relinked = 0;
    for item in pool.iter_mut().filter(|m| missing.contains(&m.id)) {
        if let Some(found) = find_exact(item, dirs).or_else(|| find_fuzzy(item, dirs)) {
            item.path = found.to_string_lossy().into_owned();
            relinked += 1;
        }
    }
    relinked
}

// ---- step 3: checking a candidate against what the project recorded -----------------------------

/// What a probe finds out about a candidate file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Probed {
    pub duration_seconds: f64,
    pub channels: Option<u16>,
    pub fps: Option<f64>,
}

/// How a candidate differs from the media it would replace.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mismatch {
    Duration { expected: f64, found: f64 },
    Channels { expected: u16, found: u16 },
    Fps { expected: f64, found: f64 },
}

impl Mismatch {
    /// The reason, in pt-BR, for the list of rejected candidates.
    pub fn describe(&self) -> String {
        match self {
            Mismatch::Duration { expected, found } => {
                format!("duração {found:.2} s em vez de {expected:.2} s")
            }
            Mismatch::Channels { expected, found } => {
                format!("{found} canal(is) em vez de {expected}")
            }
            Mismatch::Fps { expected, found } => {
                format!("{found:.3} quadros/s em vez de {expected:.3}")
            }
        }
    }
}

/// Largest difference of duration still taken for the same recording: a quarter of a second or one
/// percent, whichever is more (decoders disagree by a few frames about the end of a file).
pub fn duration_tolerance(expected: f64) -> f64 {
    (expected.abs() * 0.01).max(0.25)
}

/// Checks `found` against what the project recorded for `expected`. A trait the project did not
/// record (an older project, an unknown duration) is not checked.
pub fn validate(expected: &MediaRef, found: &Probed) -> Result<(), Mismatch> {
    if expected.duration_seconds > 0.0
        && found.duration_seconds.is_finite()
        && (found.duration_seconds - expected.duration_seconds).abs()
            > duration_tolerance(expected.duration_seconds)
    {
        return Err(Mismatch::Duration {
            expected: expected.duration_seconds,
            found: found.duration_seconds,
        });
    }
    if let Some(format) = expected.format {
        if let (Some(e), Some(f)) = (format.channels, found.channels) {
            if e != f {
                return Err(Mismatch::Channels {
                    expected: e,
                    found: f,
                });
            }
        }
        if let (Some(e), Some(f)) = (format.fps, found.fps) {
            if (e - f).abs() > 0.01 {
                return Err(Mismatch::Fps {
                    expected: e,
                    found: f,
                });
            }
        }
    }
    Ok(())
}

/// How a candidate was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    Exact,
    Fuzzy,
}

/// Most candidates looked at per kind of match, so a folder of look-alikes cannot stall the search.
pub const MAX_CANDIDATES: usize = 8;

/// Every file in `dirs` that could be `media`: the exact-name ones first, then the fuzzy ones.
pub fn candidates_for(media: &MediaRef, dirs: &[PathBuf]) -> Vec<(PathBuf, Match)> {
    let name = file_name(&media.path);
    let wanted_exact = name.to_lowercase();
    let wanted_stem = normalize(stem(name));
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    let all = candidates(dirs);
    let mut found: Vec<(PathBuf, Match)> = all
        .iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().to_lowercase() == wanted_exact)
        })
        .take(MAX_CANDIDATES)
        .map(|p| (p.clone(), Match::Exact))
        .collect();
    if !wanted_stem.is_empty() {
        let fuzzy: Vec<(PathBuf, Match)> = all
            .iter()
            .filter(|p| {
                let same_ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()) == ext;
                let cand = normalize(
                    &p.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                );
                same_ext
                    && !cand.is_empty()
                    && (cand.contains(&wanted_stem) || wanted_stem.contains(&cand))
                    && !found.iter().any(|(f, _)| f == *p)
            })
            .take(MAX_CANDIDATES)
            .map(|p| (p.clone(), Match::Fuzzy))
            .collect();
        found.extend(fuzzy);
    }
    found
}

/// A candidate that was looked at and not taken.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    pub path: PathBuf,
    pub reason: String,
}

/// What the search for one missing media came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// A file to relink to. `verified` is `false` when its metadata could not be read, which is
    /// only accepted for an exact name.
    Relinked {
        path: PathBuf,
        how: Match,
        verified: bool,
    },
    /// Nothing fit; the candidates that were looked at are listed with why.
    Unresolved { rejected: Vec<Rejected> },
}

/// Looks for `media` in `dirs` and checks every candidate with `probe` (which reads the metadata of
/// a file; it may be slow, so the caller runs this on a worker thread). The first candidate that
/// matches what the project recorded wins.
pub fn resolve(
    media: &MediaRef,
    dirs: &[PathBuf],
    probe: &mut dyn FnMut(&Path) -> Option<Probed>,
) -> Outcome {
    resolve_candidates(media, candidates_for(media, dirs), probe)
}

/// [`resolve`] over candidates that were already found (by [`search_tree`], for instance).
pub fn resolve_candidates(
    media: &MediaRef,
    candidates: Vec<(PathBuf, Match)>,
    probe: &mut dyn FnMut(&Path) -> Option<Probed>,
) -> Outcome {
    let mut rejected = Vec::new();
    for (path, how) in candidates {
        match probe(&path) {
            Some(found) => match validate(media, &found) {
                Ok(()) => {
                    return Outcome::Relinked {
                        path,
                        how,
                        verified: true,
                    }
                }
                Err(mismatch) => rejected.push(Rejected {
                    path,
                    reason: mismatch.describe(),
                }),
            },
            None if how == Match::Exact => {
                return Outcome::Relinked {
                    path,
                    how,
                    verified: false,
                }
            }
            None => rejected.push(Rejected {
                path,
                reason: "não foi possível ler a mídia".into(),
            }),
        }
    }
    Outcome::Unresolved { rejected }
}

// ---- searching everywhere -------------------------------------------------------------------------

/// Folders a search of whole disks never goes into: system and trash folders, caches and build
/// output, where a lost media file is not going to be.
const SKIPPED_FOLDERS: [&str; 12] = [
    "windows",
    "$recycle.bin",
    "system volume information",
    "appdata",
    "node_modules",
    ".git",
    "target",
    "proc",
    "sys",
    "dev",
    "lost+found",
    ".cache",
];

/// `true` for a folder the whole-disk search leaves alone.
pub fn is_skipped_folder(name: &str) -> bool {
    let lower = name.to_lowercase();
    SKIPPED_FOLDERS.contains(&lower.as_str())
}

/// Walks every folder under `roots` once and returns, for each media in `wanted`, the files that
/// could be it (the exact-name ones first, at most [`MAX_CANDIDATES`] of each kind). One walk serves
/// all the missing media. It does not follow links, leaves out [`is_skipped_folder`] folders, calls
/// `on_folder` with every folder it enters (for a progress line) and stops as soon as `cancel`
/// reports `true`, returning what it had.
pub fn search_tree(
    roots: &[PathBuf],
    wanted: &[MediaRef],
    cancel: &dyn Fn() -> bool,
    on_folder: &mut dyn FnMut(&Path),
) -> std::collections::HashMap<u64, Vec<(PathBuf, Match)>> {
    struct Want {
        id: u64,
        exact: String,
        stem: String,
        ext: Option<String>,
    }
    let wants: Vec<Want> = wanted
        .iter()
        .map(|m| {
            let name = file_name(&m.path);
            Want {
                id: m.id,
                exact: name.to_lowercase(),
                stem: normalize(stem(name)),
                ext: Path::new(name)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase()),
            }
        })
        .collect();
    let mut exact: std::collections::HashMap<u64, Vec<PathBuf>> = std::collections::HashMap::new();
    let mut fuzzy: std::collections::HashMap<u64, Vec<PathBuf>> = std::collections::HashMap::new();
    let mut stack: Vec<PathBuf> = roots.iter().rev().cloned().collect();
    while let Some(dir) = stack.pop() {
        if cancel() {
            break;
        }
        on_folder(&dir);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if kind.is_dir() {
                if !is_skipped_folder(&name) {
                    stack.push(path);
                }
                continue;
            }
            let lower = name.to_lowercase();
            let candidate_stem = normalize(stem(&name));
            let candidate_ext = Path::new(&name)
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase());
            for want in &wants {
                if lower == want.exact {
                    let list = exact.entry(want.id).or_default();
                    if list.len() < MAX_CANDIDATES {
                        list.push(path.clone());
                    }
                } else if !want.stem.is_empty()
                    && !candidate_stem.is_empty()
                    && candidate_ext == want.ext
                    && (candidate_stem.contains(&want.stem) || want.stem.contains(&candidate_stem))
                {
                    let list = fuzzy.entry(want.id).or_default();
                    if list.len() < MAX_CANDIDATES {
                        list.push(path.clone());
                    }
                }
            }
        }
    }
    wants
        .iter()
        .map(|w| {
            let mut found: Vec<(PathBuf, Match)> = exact
                .remove(&w.id)
                .unwrap_or_default()
                .into_iter()
                .map(|p| (p, Match::Exact))
                .collect();
            found.extend(
                fuzzy
                    .remove(&w.id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| (p, Match::Fuzzy)),
            );
            (w.id, found)
        })
        .collect()
}

// ---- copying a found file next to the project -----------------------------------------------------

/// `true` when `file` already sits directly in `dir`.
pub fn is_in_folder(file: &Path, dir: &Path) -> bool {
    let (Some(parent), Ok(dir)) = (file.parent(), dir.canonicalize()) else {
        return false;
    };
    parent.canonicalize().is_ok_and(|p| p == dir)
}

/// The name a copy of `source` gets in `dir`: its own, or `name_2.ext`, `name_3.ext` and so on when
/// that is taken (`exists` says whether a path is taken). A copy never replaces a file.
pub fn copy_target(source: &Path, dir: &Path, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "midia".into());
    let first = dir.join(&name);
    if !exists(&first) {
        return first;
    }
    let stem = Path::new(&name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    let ext = Path::new(&name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem}_{n}{ext}")))
        .find(|p| !exists(p))
        .expect("an unbounded range finds a free name")
}

/// Copies `source` into `dir` (created when needed) and returns where the copy is. A file that is
/// already in `dir` is returned as it is, and no file is ever overwritten. This can take a while for
/// a large file, so the caller runs it on a worker thread.
pub fn copy_into(source: &Path, dir: &Path) -> Result<PathBuf, String> {
    if !source.is_file() {
        return Err(format!("{} não existe mais", source.display()));
    }
    if is_in_folder(source, dir) {
        return Ok(source.to_path_buf());
    }
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("não foi possível criar a pasta {}: {e}", dir.display()))?;
    let target = copy_target(source, dir, Path::exists);
    // Written under a temporary name and renamed, so a stopped copy leaves no half file with the real name
    let partial = target.with_extension("neoveg-copia");
    std::fs::copy(source, &partial).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        format!("não foi possível copiar {}: {e}", source.display())
    })?;
    std::fs::rename(&partial, &target).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        format!("não foi possível gravar {}: {e}", target.display())
    })?;
    Ok(target)
}

/// Manual relink: points the pool entry `id` at `path` (the user chose it, so it is not checked).
/// `false` when there is no such entry.
pub fn relink_to(pool: &mut [MediaRef], id: u64, path: &Path) -> bool {
    match pool.iter_mut().find(|m| m.id == id) {
        Some(item) => {
            item.path = path.to_string_lossy().into_owned();
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("neoveg_relink_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn media(path: &str) -> MediaRef {
        MediaRef {
            subproject: false,
            format: None,
            id: 1,
            path: path.into(),
            duration_seconds: 1.0,
        }
    }

    #[test]
    fn exact_name_is_found_in_another_folder() {
        let d = dir("exact");
        std::fs::write(d.join("Entrevista.WAV"), b"x").unwrap();
        let found = find_exact(
            &media("C:\\antigo\\entrevista.wav"),
            std::slice::from_ref(&d),
        );
        assert_eq!(found, Some(d.join("Entrevista.WAV")));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn fuzzy_ignores_separators_and_suffixes() {
        let d = dir("fuzzy");
        std::fs::write(d.join("entrevista_final_v2.wav"), b"x").unwrap();
        std::fs::write(d.join("outra.wav"), b"x").unwrap();
        let found = find_fuzzy(&media("/x/Entrevista Final.wav"), std::slice::from_ref(&d));
        assert_eq!(found, Some(d.join("entrevista_final_v2.wav")));
        // different extension never matches
        assert_eq!(
            find_fuzzy(&media("/x/outra.mp3"), std::slice::from_ref(&d)),
            None
        );
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn relink_updates_only_missing_entries() {
        let d = dir("pool");
        std::fs::write(d.join("a.wav"), b"x").unwrap();
        let mut pool = vec![
            MediaRef {
                subproject: false,
                format: None,
                id: 1,
                path: "/old/a.wav".into(),
                duration_seconds: 1.0,
            },
            MediaRef {
                subproject: false,
                format: None,
                id: 2,
                path: "/old/b.wav".into(),
                duration_seconds: 1.0,
            },
        ];
        let n = relink_missing(&mut pool, &[1, 2], std::slice::from_ref(&d));
        assert_eq!(n, 1);
        assert!(pool[0].path.ends_with("a.wav") && pool[0].path.contains("neoveg_relink"));
        assert_eq!(pool[1].path, "/old/b.wav");
        let _ = std::fs::remove_dir_all(d);
    }

    fn recorded(path: &str, seconds: f64) -> MediaRef {
        MediaRef {
            subproject: false,
            format: Some(neoveg_common::project::MediaFormat {
                channels: Some(2),
                fps: None,
            }),
            id: 7,
            path: path.into(),
            duration_seconds: seconds,
        }
    }

    fn probed(seconds: f64, channels: u16) -> Probed {
        Probed {
            duration_seconds: seconds,
            channels: Some(channels),
            fps: None,
        }
    }

    #[test]
    fn a_candidate_must_keep_the_duration_the_channels_and_the_frame_rate() {
        let media = recorded("a.wav", 60.0);
        assert!(validate(&media, &probed(60.4, 2)).is_ok(), "within 1 %");
        assert!(validate(&media, &probed(60.7, 2)).is_err());
        assert!(matches!(
            validate(&media, &probed(60.0, 1)),
            Err(Mismatch::Channels {
                expected: 2,
                found: 1
            })
        ));
        // A short file gets the quarter-second floor
        let short = recorded("b.wav", 2.0);
        assert!(validate(&short, &probed(2.2, 2)).is_ok());
        assert!(validate(&short, &probed(2.4, 2)).is_err());
        // Frame rate, when both are known
        let mut video = recorded("c.mp4", 10.0);
        video.format = Some(neoveg_common::project::MediaFormat {
            channels: None,
            fps: Some(29.97),
        });
        let at = |fps| Probed {
            duration_seconds: 10.0,
            channels: None,
            fps: Some(fps),
        };
        assert!(validate(&video, &at(29.97)).is_ok());
        assert!(matches!(
            validate(&video, &at(25.0)),
            Err(Mismatch::Fps { .. })
        ));
        // Nothing recorded, nothing to check beyond the duration
        let mut old = recorded("d.wav", 5.0);
        old.format = None;
        assert!(validate(&old, &probed(5.0, 6)).is_ok());
        old.duration_seconds = 0.0;
        assert!(validate(&old, &probed(99.0, 1)).is_ok(), "unknown duration");
        let text = Mismatch::Duration {
            expected: 1.0,
            found: 2.0,
        }
        .describe();
        assert!(text.contains("duração"));
    }

    #[test]
    fn the_search_lists_exact_names_first_and_takes_the_first_candidate_that_fits() {
        let d = dir("resolve");
        // An exact name with the wrong length, and a look-alike with the right one
        std::fs::write(d.join("Entrevista.wav"), b"x").unwrap();
        std::fs::write(d.join("entrevista_final.wav"), b"x").unwrap();
        std::fs::write(d.join("outra.wav"), b"x").unwrap();
        let media = recorded("C:\\antigo\\entrevista.wav", 30.0);
        let found = candidates_for(&media, std::slice::from_ref(&d));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1, Match::Exact);
        assert_eq!(found[1].1, Match::Fuzzy);
        let mut probe = |p: &Path| {
            let name = p.file_name().unwrap().to_string_lossy().to_lowercase();
            Some(if name == "entrevista.wav" {
                probed(12.0, 2)
            } else {
                probed(30.1, 2)
            })
        };
        match resolve(&media, std::slice::from_ref(&d), &mut probe) {
            Outcome::Relinked {
                path,
                how,
                verified,
            } => {
                assert_eq!(path, d.join("entrevista_final.wav"));
                assert_eq!(how, Match::Fuzzy);
                assert!(verified);
            }
            other => panic!("expected a relink, got {other:?}"),
        }
        // Nothing fits: every candidate comes back with its reason
        let mut none = |_: &Path| Some(probed(1.0, 2));
        match resolve(&media, std::slice::from_ref(&d), &mut none) {
            Outcome::Unresolved { rejected } => {
                assert_eq!(rejected.len(), 2);
                assert!(rejected[0].reason.contains("duração"));
            }
            other => panic!("expected no relink, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_taken_only_for_an_exact_name() {
        let d = dir("unreadable");
        std::fs::write(d.join("clip.mp4"), b"x").unwrap();
        std::fs::write(d.join("clip2.mp4"), b"x").unwrap();
        let mut unreadable = |_: &Path| None;
        let exact = recorded("x\\clip.mp4", 5.0);
        assert!(matches!(
            resolve(&exact, std::slice::from_ref(&d), &mut unreadable),
            Outcome::Relinked {
                verified: false,
                how: Match::Exact,
                ..
            }
        ));
        let fuzzy = recorded("x\\clip_antigo.mp4", 5.0);
        let found = candidates_for(&fuzzy, std::slice::from_ref(&d));
        assert!(found.iter().all(|(_, how)| *how == Match::Fuzzy));
        assert!(matches!(
            resolve(&fuzzy, std::slice::from_ref(&d), &mut unreadable),
            Outcome::Unresolved { .. }
        ));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_folder_listed_twice_gives_each_candidate_once() {
        let d = dir("twice");
        std::fs::write(d.join("voz.wav"), b"x").unwrap();
        let media = recorded("x/voz.wav", 1.0);
        let found = candidates_for(&media, &[d.clone(), d.clone()]);
        assert_eq!(found.len(), 1);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_manual_relink_points_the_entry_at_the_chosen_file() {
        let mut pool = vec![recorded("old.wav", 3.0)];
        assert!(relink_to(&mut pool, 7, Path::new("C:\\novo\\new.wav")));
        assert_eq!(pool[0].path, "C:\\novo\\new.wav");
        assert!(!relink_to(&mut pool, 99, Path::new("x.wav")));
    }

    #[test]
    fn one_walk_finds_the_candidates_of_every_missing_media_and_leaves_system_folders_alone() {
        let root = dir("tree");
        let deep = root.join("projetos").join("2024").join("audio");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(root.join("Windows")).unwrap();
        std::fs::create_dir_all(root.join("node_modules")).unwrap();
        std::fs::write(deep.join("Voz.wav"), b"x").unwrap();
        std::fs::write(deep.join("trilha_final.mp3"), b"x").unwrap();
        std::fs::write(root.join("Windows").join("voz.wav"), b"x").unwrap();
        std::fs::write(root.join("node_modules").join("trilha.mp3"), b"x").unwrap();
        let wanted = vec![
            MediaRef {
                id: 1,
                ..recorded("C:\\antigo\\voz.wav", 3.0)
            },
            MediaRef {
                id: 2,
                ..recorded("C:\\antigo\\trilha.mp3", 3.0)
            },
            MediaRef {
                id: 3,
                ..recorded("C:\\antigo\\nunca_existiu.wav", 3.0)
            },
        ];
        let mut visited = 0;
        let found = search_tree(std::slice::from_ref(&root), &wanted, &|| false, &mut |_| {
            visited += 1
        });
        assert_eq!(found[&1], vec![(deep.join("Voz.wav"), Match::Exact)]);
        assert_eq!(
            found[&2],
            vec![(deep.join("trilha_final.mp3"), Match::Fuzzy)]
        );
        assert!(found[&3].is_empty(), "nothing with that name anywhere");
        // The root, projetos, 2024 and audio: Windows and node_modules are left out
        assert_eq!(visited, 4);
        // Candidates go on to the same check as any other
        let mut probe = |_: &Path| Some(probed(3.0, 2));
        let outcome = resolve_candidates(&wanted[0], found[&1].clone(), &mut probe);
        assert!(matches!(
            outcome,
            Outcome::Relinked {
                how: Match::Exact,
                ..
            }
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_cancelled_walk_stops_and_keeps_what_it_had() {
        let root = dir("cancel");
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::write(root.join("voz.wav"), b"x").unwrap();
        let wanted = vec![MediaRef {
            id: 1,
            ..recorded("C:\\x\\voz.wav", 3.0)
        }];
        let entered = std::cell::Cell::new(0);
        // Cancel after the first folder: the files of that folder are kept, the sub-folders are not walked
        let found = search_tree(
            std::slice::from_ref(&root),
            &wanted,
            &|| entered.get() >= 1,
            &mut |_| entered.set(entered.get() + 1),
        );
        assert_eq!(entered.get(), 1);
        assert_eq!(found[&1].len(), 1);
        // Every media has an entry even when the walk is cut short
        assert!(
            search_tree(std::slice::from_ref(&root), &wanted, &|| true, &mut |_| {})[&1].is_empty()
        );
        assert!(is_skipped_folder("$Recycle.Bin") && !is_skipped_folder("Meus Videos"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_copy_gets_its_own_name_or_the_next_free_one_and_never_replaces_a_file() {
        let dir = Path::new("projeto");
        let taken = [dir.join("voz.wav"), dir.join("voz_2.wav")];
        let exists = |p: &Path| taken.contains(&p.to_path_buf());
        assert_eq!(
            copy_target(Path::new("C:\\fora\\take.wav"), dir, exists),
            dir.join("take.wav")
        );
        assert_eq!(
            copy_target(Path::new("C:\\fora\\voz.wav"), dir, exists),
            dir.join("voz_3.wav")
        );
        // No extension
        assert_eq!(
            copy_target(Path::new("sem_ext"), dir, |p| p == dir.join("sem_ext")),
            dir.join("sem_ext_2")
        );
    }

    #[test]
    fn copying_puts_the_file_in_the_folder_leaves_the_original_and_skips_a_file_already_there() {
        let from = dir("copy_from");
        let to = dir("copy_to");
        std::fs::write(from.join("voz.wav"), b"som").unwrap();
        // The folder is created when it is missing
        let nested = to.join("midia");
        let copy = copy_into(&from.join("voz.wav"), &nested).unwrap();
        assert_eq!(copy, nested.join("voz.wav"));
        assert_eq!(std::fs::read(&copy).unwrap(), b"som");
        assert!(from.join("voz.wav").exists(), "the original stays");
        // The same name again does not replace the first
        std::fs::write(from.join("voz.wav"), b"outro").unwrap();
        let second = copy_into(&from.join("voz.wav"), &nested).unwrap();
        assert_eq!(second, nested.join("voz_2.wav"));
        assert_eq!(std::fs::read(&copy).unwrap(), b"som");
        // A file that already lives there is not copied onto itself
        assert!(is_in_folder(&copy, &nested));
        assert_eq!(copy_into(&copy, &nested).unwrap(), copy);
        assert!(!is_in_folder(&from.join("voz.wav"), &nested));
        // Nothing half-written is left, and a missing source is an error
        assert!(std::fs::read_dir(&nested).unwrap().all(|e| !e
            .unwrap()
            .path()
            .to_string_lossy()
            .ends_with(".neoveg-copia")));
        assert!(copy_into(&from.join("nao_existe.wav"), &nested).is_err());
        let _ = std::fs::remove_dir_all(from);
        let _ = std::fs::remove_dir_all(to);
    }
}
