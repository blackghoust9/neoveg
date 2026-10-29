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
    let mut rejected = Vec::new();
    for (path, how) in candidates_for(media, dirs) {
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
}
