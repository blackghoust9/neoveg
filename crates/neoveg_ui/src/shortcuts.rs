//! Conversion between the toolkit-independent [`Shortcut`] and egui key events.

use eframe::egui::{Key, Modifiers};
use neoveg_common::keymap::{normalize_key, Shortcut};

/// Canonical key name of an egui key, or `None` when the key cannot be bound.
pub fn key_name(key: Key) -> Option<String> {
    let raw = match key {
        // `symbol_or_name` would return arrow glyphs and a typographic minus
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => key.name(),
        Key::Minus => "-",
        other => other.symbol_or_name(),
    };
    normalize_key(raw)
}

/// The shortcut produced by pressing `key` with `modifiers`.
pub fn from_egui(key: Key, modifiers: Modifiers) -> Option<Shortcut> {
    // `Shift+=` types a plus on most keyboards and reaches egui as the Plus key: it is still the key
    // of the equals sign, so the shortcut is spelled `Shift+=`
    let key = if key == Key::Plus && modifiers.shift {
        Key::Equals
    } else {
        key
    };
    Shortcut::new(
        modifiers.command,
        modifiers.shift,
        modifiers.alt,
        &key_name(key)?,
    )
}

/// The egui modifiers and key matching `shortcut`.
pub fn to_egui(shortcut: &Shortcut) -> Option<(Modifiers, Key)> {
    let key = Key::ALL
        .iter()
        .copied()
        .find(|k| key_name(*k).as_deref() == Some(shortcut.key.as_str()))?;
    let mut modifiers = Modifiers::NONE;
    if shortcut.ctrl {
        modifiers = modifiers | Modifiers::COMMAND;
    }
    if shortcut.shift {
        modifiers = modifiers | Modifiers::SHIFT;
    }
    if shortcut.alt {
        modifiers = modifiers | Modifiers::ALT;
    }
    Some((modifiers, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoveg_common::commands::Command;
    use neoveg_common::keymap::KeyMap;

    #[test]
    fn shift_and_the_plus_key_make_the_shortcut_of_the_equals_key() {
        let shifted = from_egui(Key::Plus, Modifiers::SHIFT).expect("a shortcut");
        assert_eq!(shifted.to_string(), "Shift+=");
        assert_eq!(
            from_egui(Key::Equals, Modifiers::SHIFT)
                .unwrap()
                .to_string(),
            "Shift+="
        );
        // A plus without Shift (the numpad one) is not the equals key
        assert!(from_egui(Key::Plus, Modifiers::NONE).is_none());
        assert_eq!(
            from_egui(Key::Minus, Modifiers::SHIFT).unwrap().to_string(),
            "Shift+-"
        );
    }

    #[test]
    fn every_default_shortcut_can_be_matched_in_egui() {
        let map = KeyMap::defaults();
        for &command in Command::ALL {
            for shortcut in map.shortcuts_for(command) {
                assert!(
                    to_egui(&shortcut).is_some(),
                    "{shortcut} ({}) has no egui key",
                    command.id()
                );
            }
        }
    }

    #[test]
    fn egui_round_trip_keeps_the_shortcut() {
        let map = KeyMap::defaults();
        for &command in Command::ALL {
            for shortcut in map.shortcuts_for(command) {
                let (modifiers, key) = to_egui(&shortcut).unwrap();
                assert_eq!(from_egui(key, modifiers), Some(shortcut));
            }
        }
    }

    #[test]
    fn modifier_keys_alone_are_not_shortcuts() {
        assert_eq!(key_name(Key::Copy), None);
        assert_eq!(key_name(Key::F20), None);
        assert_eq!(key_name(Key::ArrowLeft).as_deref(), Some("Left"));
        assert_eq!(key_name(Key::Minus).as_deref(), Some("-"));
        assert_eq!(key_name(Key::OpenBracket).as_deref(), Some("["));
    }
}
