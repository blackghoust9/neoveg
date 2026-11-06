//! *Trocar arquivos de mídia por pasta*: the user picks a folder and every media of the project whose
//! file name is there is pointed at the file in that folder (the rule is `neoveg_engine::relink::
//! replace_by_folder`). One status line says how many were swapped.

use neoveg_common::i18n::tr_args;
use neoveg_engine::relink;

use super::*;

impl AppState {
    pub(super) fn replace_media_by_folder(&mut self, ctx: &egui::Context) {
        let Some(dir) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        self.swap_media_to_folder(&dir, ctx);
    }

    /// Points the media found in `dir` at it and loads them again.
    pub(super) fn swap_media_to_folder(&mut self, dir: &std::path::Path, ctx: &egui::Context) -> usize {
        let changed = relink::replace_by_folder(&mut self.project.media_pool, dir);
        for (id, path) in &changed {
            self.missing.retain(|m| m.id != *id);
            self.spawn_media(path.clone(), Some(*id), ctx);
        }
        if changed.is_empty() {
            self.status = tr_args(
                "Nenhuma mídia do projeto tem arquivo com o mesmo nome em {}",
                &[&dir.display()],
            );
        } else {
            self.forced_dirty = true;
            self.status = tr_args(
                "{} mídias trocadas por arquivos de {}",
                &[&changed.len(), &dir.display()],
            );
        }
        changed.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_are_swapped_to_the_files_of_a_folder() {
        let dir = std::env::temp_dir().join(format!("neoveg_swap_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("clip.wav"), b"x").unwrap();
        let mut s = AppState::new();
        s.project.media_pool.push(MediaRef {
            subproject: false,
            format: None,
            id: 7,
            path: "C:/elsewhere/clip.wav".into(),
            duration_seconds: 1.0,
        });
        let ctx = egui::Context::default();
        assert_eq!(s.swap_media_to_folder(&dir, &ctx), 1);
        assert!(s.project.media_pool[0].path.contains("neoveg_swap_"));
        assert!(s.forced_dirty);
        assert_eq!(s.swap_media_to_folder(&dir, &ctx), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
