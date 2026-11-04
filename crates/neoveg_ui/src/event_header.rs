//! The header of an event on the timeline (manual v21 p.242): the name on the left and, on the right,
//! small buttons: *Generated Media* (for generated media), *Pan/Crop* (video events), *FX* and the
//! menu (☰) that opens the cascading menu of the event. This module is geometry only: which buttons an
//! event has, where they sit and which one a pointer is over. The painting and the actions live in
//! `app.rs`.

use eframe::egui::{pos2, vec2, Pos2, Rect};
use neoveg_engine::TrackKind;

/// Side of a button, in points.
pub const BUTTON_PX: f32 = 12.0;
/// Space between two buttons.
pub const GAP_PX: f32 = 2.0;
/// Room kept at the right edge of the event for the fade-out handle.
pub const RIGHT_MARGIN_PX: f32 = 16.0;
/// Room kept for the name: a header that cannot give it this much has fewer buttons.
pub const MIN_NAME_PX: f32 = 36.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderButton {
    /// Edit the generated media the event plays.
    GeneratedMedia,
    /// Open the Pan/Crop of the event.
    PanCrop,
    /// Show the effects of the event (video) or of its track (audio).
    Fx,
    /// The cascading menu of the event (the same one as a right click).
    Menu,
}

impl HeaderButton {
    /// What the button does, for its tooltip (pt-BR).
    pub fn tooltip(self, video: bool) -> &'static str {
        match self {
            HeaderButton::GeneratedMedia => "Editar a mídia gerada",
            HeaderButton::PanCrop => "Pan/Crop do evento",
            HeaderButton::Fx if video => "Efeitos de vídeo do evento",
            HeaderButton::Fx => "Efeitos de áudio da faixa",
            HeaderButton::Menu => "Mais ferramentas do evento",
        }
    }
}

/// The buttons an event has, left to right. Audio events have no Pan/Crop, an adjustment event is
/// only effects and the menu, and the generated-media button only exists for generated media.
pub fn buttons_for(kind: TrackKind, generated: bool, adjustment: bool) -> Vec<HeaderButton> {
    let mut list = Vec::new();
    if generated && kind == TrackKind::Video {
        list.push(HeaderButton::GeneratedMedia);
    }
    if kind == TrackKind::Video && !adjustment {
        list.push(HeaderButton::PanCrop);
    }
    list.push(HeaderButton::Fx);
    list.push(HeaderButton::Menu);
    list
}

/// Where the buttons sit in the header strip of an event spanning `ev_rect`, right aligned. When the
/// event is too narrow for all of them the leftmost ones are left out (the menu is the last to go),
/// and a very narrow event has none.
pub fn layout(
    ev_rect: Rect,
    buttons: &[HeaderButton],
    header_height: f32,
) -> Vec<(HeaderButton, Rect)> {
    let room = ev_rect.width() - RIGHT_MARGIN_PX - MIN_NAME_PX;
    let fit = if room < BUTTON_PX {
        0
    } else {
        (((room + GAP_PX) / (BUTTON_PX + GAP_PX)).floor() as usize).min(buttons.len())
    };
    let shown = &buttons[buttons.len() - fit..];
    let top = ev_rect.top() + ((header_height - BUTTON_PX) / 2.0).max(0.0);
    let mut right = ev_rect.right() - RIGHT_MARGIN_PX;
    let mut placed: Vec<(HeaderButton, Rect)> = Vec::with_capacity(fit);
    for button in shown.iter().rev() {
        let rect = Rect::from_min_size(pos2(right - BUTTON_PX, top), vec2(BUTTON_PX, BUTTON_PX));
        placed.push((*button, rect));
        right -= BUTTON_PX + GAP_PX;
    }
    placed.reverse();
    placed
}

/// The button under `p`, if any.
pub fn hit(placed: &[(HeaderButton, Rect)], p: Pos2) -> Option<HeaderButton> {
    placed
        .iter()
        .find(|(_, rect)| rect.expand(1.0).contains(p))
        .map(|(button, _)| *button)
}

/// The left edge of the first button (where the name has to stop), or the right edge of the event
/// when there are none.
pub fn name_limit(ev_rect: Rect, placed: &[(HeaderButton, Rect)]) -> f32 {
    placed
        .first()
        .map_or(ev_rect.right(), |(_, rect)| rect.left() - GAP_PX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(width: f32) -> Rect {
        Rect::from_min_size(pos2(100.0, 50.0), vec2(width, 60.0))
    }

    #[test]
    fn each_kind_of_event_has_its_own_buttons() {
        use HeaderButton::*;
        assert_eq!(
            buttons_for(TrackKind::Video, false, false),
            [PanCrop, Fx, Menu]
        );
        assert_eq!(
            buttons_for(TrackKind::Video, true, false),
            [GeneratedMedia, PanCrop, Fx, Menu]
        );
        assert_eq!(buttons_for(TrackKind::Audio, false, false), [Fx, Menu]);
        assert_eq!(buttons_for(TrackKind::Video, false, true), [Fx, Menu]);
        // Generated media is a video thing
        assert_eq!(buttons_for(TrackKind::Audio, true, false), [Fx, Menu]);
    }

    #[test]
    fn the_buttons_sit_at_the_right_of_the_header_and_never_overlap() {
        let all = buttons_for(TrackKind::Video, true, false);
        let placed = layout(event(400.0), &all, 14.0);
        assert_eq!(placed.len(), 4);
        let right = 100.0 + 400.0 - RIGHT_MARGIN_PX;
        assert!(
            (placed[3].1.right() - right).abs() < 1e-3,
            "the menu is the last, at the margin"
        );
        for pair in placed.windows(2) {
            assert!(
                pair[0].1.right() + GAP_PX <= pair[1].1.left() + 1e-3,
                "{pair:?}"
            );
        }
        for (_, rect) in &placed {
            // Inside the header strip
            assert!(rect.top() >= 50.0 && rect.bottom() <= 50.0 + 14.0 + 1e-3);
        }
        assert_eq!(placed[0].0, HeaderButton::GeneratedMedia);
        assert!(name_limit(event(400.0), &placed) < placed[0].1.left());
    }

    #[test]
    fn a_narrow_event_loses_buttons_from_the_left_and_a_tiny_one_has_none() {
        let all = buttons_for(TrackKind::Video, true, false);
        let widths: Vec<usize> = [40.0, 70.0, 84.0, 100.0, 120.0, 400.0]
            .iter()
            .map(|w| layout(event(*w), &all, 14.0).len())
            .collect();
        assert_eq!(widths[0], 0, "no room for a name and a button");
        assert!(
            widths.windows(2).all(|p| p[0] <= p[1]),
            "more room, more buttons: {widths:?}"
        );
        assert_eq!(*widths.last().unwrap(), 4);
        // What stays is the end of the list: the menu first
        let two = layout(event(84.0), &all, 14.0);
        assert!(two.last().is_some_and(|(b, _)| *b == HeaderButton::Menu));
        let none = layout(event(30.0), &all, 14.0);
        assert!(none.is_empty());
        assert_eq!(name_limit(event(30.0), &none), 130.0);
    }

    #[test]
    fn the_pointer_finds_the_button_under_it_and_the_gaps_belong_to_the_event() {
        let all = buttons_for(TrackKind::Video, false, false);
        let placed = layout(event(300.0), &all, 14.0);
        let menu = placed.last().unwrap().1;
        assert_eq!(hit(&placed, menu.center()), Some(HeaderButton::Menu));
        let fx = placed[1].1;
        assert_eq!(hit(&placed, fx.center()), Some(HeaderButton::Fx));
        // Left of every button, and below the header
        assert_eq!(hit(&placed, pos2(110.0, 55.0)), None);
        assert_eq!(hit(&placed, pos2(menu.center().x, 50.0 + 40.0)), None);
        assert!(!HeaderButton::Fx.tooltip(true).is_empty());
        assert_ne!(
            HeaderButton::Fx.tooltip(true),
            HeaderButton::Fx.tooltip(false)
        );
    }
}
