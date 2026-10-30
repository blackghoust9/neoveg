//! The edge frames of an event while it is slipped or slid: the Video Preview shows, side by side,
//! the first and the last frame the event shows (as the drag changes them) until the mouse is
//! released. The two source frames are asked from the frame worker, once for each frame number, and
//! kept as textures only while the drag lasts.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Align2, Color32, FontId, Painter, Rect};
use neoveg_common::i18n::tr;
use neoveg_video::frame::FrameBuffer;

/// Widest frame decoded for the preview halves.
pub const MAX_WIDTH: u32 = 640;

#[derive(Default)]
pub struct EdgeFrames {
    /// What each half waits for or shows: the file and the source time (a whole frame).
    keys: [Option<(PathBuf, f64)>; 2],
    textures: [Option<egui::TextureHandle>; 2],
}

impl EdgeFrames {
    /// `true` while the halves are on screen.
    pub fn visible(&self) -> bool {
        self.keys.iter().any(Option::is_some)
    }

    /// Forgets the frames (the drag ended or the feature was turned off).
    pub fn clear(&mut self) {
        self.keys = [None, None];
        self.textures = [None, None];
    }

    /// Sets what the two halves should show and returns the requests that are new: a half that
    /// already shows (or waits for) the same frame asks for nothing.
    pub fn want(&mut self, wanted: [(PathBuf, f64); 2]) -> Vec<(PathBuf, f64)> {
        let mut requests = Vec::new();
        for (slot, key) in wanted.into_iter().enumerate() {
            if self.keys[slot].as_ref() != Some(&key) {
                requests.push(key.clone());
                self.keys[slot] = Some(key);
            }
        }
        requests
    }

    /// The half that waits for the frame of `path` at `seconds`, if any.
    pub fn slot_for(&self, path: &Path, seconds: f64) -> Option<usize> {
        self.keys.iter().position(|key| {
            key.as_ref()
                .is_some_and(|(p, s)| p == path && (*s - seconds).abs() < 1e-9)
        })
    }

    /// Puts a decoded frame in `slot`. The picture already there stays until this one arrives, so the
    /// halves do not flicker while the mouse moves.
    pub fn set(&mut self, ctx: &egui::Context, slot: usize, frame: &FrameBuffer) {
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.rgba,
        );
        let options = egui::TextureOptions::LINEAR;
        match &mut self.textures[slot] {
            Some(texture) => texture.set(image, options),
            None => {
                let name = if slot == 0 { "edge_first" } else { "edge_last" };
                self.textures[slot] = Some(ctx.load_texture(name, image, options));
            }
        }
    }

    /// Draws the halves over `frame_rect` (the picture area of the preview), each labelled with its
    /// caption and timecode. `aspect` is the width over the height of the project picture.
    pub fn paint(&self, painter: &Painter, frame_rect: Rect, aspect: f32, labels: &[String; 2]) {
        if !self.visible() {
            return;
        }
        painter.rect_filled(frame_rect, 0.0, Color32::from_black_alpha(235));
        let (left, right) = frame_rect.split_left_right_at_fraction(0.5);
        let captions = [tr("Primeiro quadro"), tr("Último quadro")];
        for (slot, half) in [left, right].into_iter().enumerate() {
            let room = half.shrink(6.0);
            let width = room.width().min(room.height() * aspect).max(1.0);
            let picture = Rect::from_center_size(
                room.center(),
                egui::vec2(width, (width / aspect.max(0.01)).max(1.0)),
            );
            painter.rect_filled(picture, 0.0, Color32::BLACK);
            if let Some(texture) = &self.textures[slot] {
                painter.image(
                    texture.id(),
                    picture,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            painter.text(
                picture.left_top() + egui::vec2(4.0, 3.0),
                Align2::LEFT_TOP,
                format!("{} · {}", captions[slot], labels[slot]),
                FontId::monospace(11.0),
                Color32::from_rgb(0xF1, 0xC4, 0x0F),
            );
        }
        painter.line_segment(
            [frame_rect.center_top(), frame_rect.center_bottom()],
            egui::Stroke::new(1.0_f32, Color32::from_gray(90)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str, seconds: f64) -> (PathBuf, f64) {
        (PathBuf::from(name), seconds)
    }

    #[test]
    fn only_a_changed_half_asks_for_a_new_frame_and_replies_find_their_half() {
        let mut edge = EdgeFrames::default();
        assert!(!edge.visible());
        let first = edge.want([key("a.mp4", 1.0), key("a.mp4", 5.0)]);
        assert_eq!(first, vec![key("a.mp4", 1.0), key("a.mp4", 5.0)]);
        assert!(edge.visible());
        // The same frames again: nothing to ask. One half moved: only it is asked for
        assert!(edge.want([key("a.mp4", 1.0), key("a.mp4", 5.0)]).is_empty());
        assert_eq!(
            edge.want([key("a.mp4", 1.0), key("a.mp4", 5.5)]),
            vec![key("a.mp4", 5.5)]
        );
        assert_eq!(edge.slot_for(Path::new("a.mp4"), 1.0), Some(0));
        assert_eq!(edge.slot_for(Path::new("a.mp4"), 5.5), Some(1));
        assert_eq!(
            edge.slot_for(Path::new("a.mp4"), 5.0),
            None,
            "a stale reply"
        );
        assert_eq!(edge.slot_for(Path::new("b.mp4"), 1.0), None);
        edge.clear();
        assert!(!edge.visible());
        assert_eq!(edge.slot_for(Path::new("a.mp4"), 1.0), None);
    }
}
