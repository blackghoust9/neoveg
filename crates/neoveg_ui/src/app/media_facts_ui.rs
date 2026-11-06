//! Glue for the audio and video properties shown in the Media Properties window and for the event
//! length shown while trimming. The wording and the rules are in `neoveg_engine::media_facts`.

use neoveg_engine::media_facts::{multiple_lines, MediaFacts};

use super::AppState;

impl AppState {
    /// What is known about media `id`: size and rate of the picture, rate and channels of the sound,
    /// length and size of the file.
    pub(super) fn media_facts(&self, id: u64) -> Option<MediaFacts> {
        let item = self.project.media_pool.iter().find(|m| m.id == id)?;
        let name = std::path::Path::new(&item.path)
            .file_name()
            .map_or_else(|| item.path.clone(), |n| n.to_string_lossy().into_owned());
        let video = self.video_info.get(&id);
        let sound = self.media.get(&id);
        Some(MediaFacts {
            name,
            duration: video
                .map(|v| v.duration)
                .or_else(|| sound.map(|m| m.duration))
                .unwrap_or(item.duration_seconds),
            video: video.map(|v| (v.width, v.height, v.fps)),
            audio: sound.map(|m| (m.sample_rate, m.channels)),
            file_bytes: std::fs::metadata(&item.path).ok().map(|m| m.len()),
        })
    }

    /// The text of the properties of `ids` (one file, or several at once), one fact per line.
    pub(super) fn media_summary_text(&self, ids: &[u64]) -> String {
        let facts: Vec<MediaFacts> = ids.iter().filter_map(|id| self.media_facts(*id)).collect();
        if facts.is_empty() {
            return String::new();
        }
        multiple_lines(&facts).join("\n")
    }

    /// Shows the length of the event being trimmed in the status bar.
    pub(super) fn show_trim_length(&mut self, new_duration: f64, old_duration: f64) {
        let fps = self.project.settings.fps;
        self.status = neoveg_engine::media_facts::trim_length_label(
            new_duration,
            old_duration,
            fps,
            self.drop_frame(),
        );
    }
}
