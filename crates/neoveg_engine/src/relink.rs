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
    dirs.iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|rd| rd.filter_map(Result::ok))
        .map(|e| e.path())
        .filter(|p| p.is_file())
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
                id: 1,
                path: "/old/a.wav".into(),
                duration_seconds: 1.0,
            },
            MediaRef {
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
}
