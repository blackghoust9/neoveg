//! Keyboard map: shortcuts, the default bindings and user overrides.
//!
//! A [`Shortcut`] is independent of any UI toolkit (a modifier set plus a key name), so the map can
//! be parsed, validated, saved and unit-tested without a window. The UI crate converts key events
//! to shortcuts and runs the bound [`Command`].
//!
//! User changes are stored in `preferences.json` as `command id -> [shortcut strings]` and only for
//! the commands that differ from the defaults, so improving a default later reaches everyone who
//! never touched that command. An empty list means "unbound".

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::commands::Command;

/// Key names accepted in shortcut strings (compared case-insensitively on input).
const NAMED_KEYS: &[&str] = &[
    "Space",
    "Enter",
    "Escape",
    "Tab",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Left",
    "Right",
    "Up",
    "Down",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
];

/// Punctuation keys; the layout-independent names are what the UI reports.
const SYMBOL_KEYS: &[&str] = &["[", "]", "\\", ",", ".", "=", "-", "`", "/", ";", "'"];

/// Canonical spelling of a key name, or `None` when the key is not supported.
pub fn normalize_key(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.chars().count() == 1 {
        let ch = raw.chars().next()?;
        if ch.is_ascii_alphabetic() {
            return Some(ch.to_ascii_uppercase().to_string());
        }
        if ch.is_ascii_digit() || SYMBOL_KEYS.contains(&raw) {
            return Some(raw.to_string());
        }
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    // Common aliases from the reference manuals
    let alias = match lower.as_str() {
        "esc" => "escape",
        "return" => "enter",
        "del" => "delete",
        "ins" => "insert",
        "pgup" | "page up" => "pageup",
        "pgdn" | "page down" => "pagedown",
        "arrowleft" | "left arrow" => "left",
        "arrowright" | "right arrow" => "right",
        "arrowup" | "up arrow" => "up",
        "arrowdown" | "down arrow" => "down",
        other => other,
    };
    NAMED_KEYS
        .iter()
        .find(|k| k.to_ascii_lowercase() == alias)
        .map(|k| (*k).to_string())
}

/// A key together with the modifiers that must be held (`Ctrl` is `Cmd` on macOS).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Shortcut {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: String,
}

impl Shortcut {
    /// Builds a shortcut from a key name; `None` when the key is unsupported.
    pub fn new(ctrl: bool, shift: bool, alt: bool, key: &str) -> Option<Self> {
        Some(Self {
            ctrl,
            shift,
            alt,
            key: normalize_key(key)?,
        })
    }

    /// Number of modifiers; shortcuts with more modifiers must be tested first because a
    /// toolkit may report `Ctrl+Shift+Q` as also matching `Ctrl+Q`.
    pub fn modifier_count(&self) -> u8 {
        u8::from(self.ctrl) + u8::from(self.shift) + u8::from(self.alt)
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        f.write_str(&self.key)
    }
}

/// Why a shortcut string was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseShortcutError(pub String);

impl fmt::Display for ParseShortcutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseShortcutError {}

impl FromStr for Shortcut {
    type Err = ParseShortcutError;

    /// Parses `Ctrl+Shift+V`, `alt+left`, `F8`, `Ctrl+[`… (modifiers in any order).
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        if text.is_empty() {
            return Err(ParseShortcutError("atalho vazio".into()));
        }
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key = parts
            .pop()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| ParseShortcutError(format!("falta a tecla em «{text}»")))?;
        let (mut ctrl, mut shift, mut alt) = (false, false, false);
        for modifier in parts {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "command" => ctrl = true,
                "shift" => shift = true,
                "alt" | "option" => alt = true,
                other => {
                    return Err(ParseShortcutError(format!(
                        "modificador desconhecido «{other}» em «{text}»"
                    )))
                }
            }
        }
        Shortcut::new(ctrl, shift, alt, key)
            .ok_or_else(|| ParseShortcutError(format!("tecla desconhecida «{key}» em «{text}»")))
    }
}

/// Default bindings (reference: research entry 17, "Default Keyboard Map").
///
/// Numeric-keypad bindings are not included because egui does not report which physical key
/// produced a digit.
const DEFAULT_BINDINGS: &[(Command, &[&str])] = &[
    (Command::FileOpen, &["Ctrl+O"]),
    (Command::FileSave, &["Ctrl+S"]),
    (Command::FileSaveAs, &["Ctrl+Shift+S"]),
    (Command::FileNew, &["Ctrl+N"]),
    (Command::FileCreateSubproject, &["Ctrl+Shift+N"]),
    (Command::FileClose, &["Ctrl+F4"]),
    (Command::FileProperties, &["Alt+Enter"]),
    (Command::EditUndo, &["Ctrl+Z", "Alt+Backspace"]),
    (Command::EditRedo, &["Ctrl+Y", "Ctrl+Shift+Z"]),
    (Command::EditRippleDelete, &["Ctrl+Delete"]),
    (Command::EditDelete, &["Delete"]),
    (Command::EditSplit, &["S"]),
    (Command::EditCut, &["Ctrl+X", "Shift+Delete"]),
    (Command::EditCopy, &["Ctrl+C", "Ctrl+Insert"]),
    (Command::EditPaste, &["Ctrl+V", "Shift+Insert"]),
    (Command::EditPasteInsert, &["Ctrl+Shift+V"]),
    (Command::EditPasteRepeat, &["Ctrl+B"]),
    (Command::EditTrimToSelection, &["Ctrl+T"]),
    (Command::EditTrimStart, &["Alt+["]),
    (Command::EditTrimEnd, &["Alt+]"]),
    (Command::EditSelectAll, &["Ctrl+A"]),
    (Command::EditSelectNone, &["Ctrl+Shift+A"]),
    (Command::EditGroup, &["G"]),
    (Command::EditUngroup, &["U"]),
    (Command::EditIgnoreGrouping, &["Ctrl+Shift+U"]),
    (Command::EditToolNormal, &["Ctrl+D"]),
    (Command::EditToolNext, &["D"]),
    (Command::EditToolPrevious, &["Shift+D"]),
    (Command::EditNextTake, &["T"]),
    (Command::EditPreviousTake, &["Shift+T"]),
    (Command::InsertAudioTrack, &["Ctrl+Q"]),
    (Command::InsertVideoTrack, &["Ctrl+Shift+Q"]),
    (Command::InsertMarker, &["M"]),
    (Command::InsertRegion, &["R"]),
    (Command::InsertCommandMarker, &["C"]),
    (Command::RenderToNewTrack, &["Ctrl+M"]),
    // One frame per arrow key is kept from the first Neoveg releases; the reference map uses
    // Alt+Left/Right for it, so both are bound.
    (Command::CursorLeftByFrame, &["Left", "Alt+Left"]),
    (Command::CursorRightByFrame, &["Right", "Alt+Right"]),
    (Command::CursorProjectStart, &["Ctrl+Home", "W"]),
    (Command::CursorProjectEnd, &["Ctrl+End"]),
    (Command::CursorSelectionStart, &["Home"]),
    (Command::CursorSelectionEnd, &["End"]),
    (Command::CursorPreviousMarker, &["Ctrl+Left"]),
    (Command::CursorNextMarker, &["Ctrl+Right"]),
    (Command::CursorMarker1, &["1"]),
    (Command::CursorMarker2, &["2"]),
    (Command::CursorMarker3, &["3"]),
    (Command::CursorMarker4, &["4"]),
    (Command::CursorMarker5, &["5"]),
    (Command::CursorMarker6, &["6"]),
    (Command::CursorMarker7, &["7"]),
    (Command::CursorMarker8, &["8"]),
    (Command::CursorMarker9, &["9"]),
    (Command::CursorMarker10, &["0"]),
    (Command::CursorPreviousEdit, &["Ctrl+Alt+Left"]),
    (Command::CursorNextEdit, &["Ctrl+Alt+Right"]),
    (Command::CursorGoTo, &["Ctrl+G"]),
    (Command::CursorCenterView, &["\\"]),
    (Command::SelectionSetIn, &["I"]),
    (Command::SelectionSetOut, &["O"]),
    (Command::SelectionLoopRegion, &["Shift+Q"]),
    // Enter also plays/pauses in the reference map; it is left unbound here because Enter
    // activates a focused button in egui.
    (Command::TransportPlayToggle, &["Space"]),
    (Command::TransportShuttleReverse, &["J"]),
    (Command::TransportShuttlePause, &["K"]),
    (Command::TransportShuttleForward, &["L"]),
    (Command::TransportStop, &["Escape"]),
    (Command::TransportPlayFromStart, &["Shift+Space"]),
    (Command::TransportLoop, &["Q"]),
    (Command::TransportRecord, &["Ctrl+R"]),
    (Command::OptionsSnapping, &["F8"]),
    (Command::OptionsAutoCrossfade, &["Ctrl+Shift+X"]),
    (Command::OptionsSnapToGrid, &["Ctrl+F8"]),
    (Command::OptionsSnapToMarkers, &["Shift+F8"]),
    (Command::OptionsSnapToAllEvents, &["Ctrl+Shift+F8"]),
    (Command::OptionsQuantizeToFrames, &["Alt+F8"]),
    (Command::OptionsAutoRipple, &["Ctrl+L"]),
    (Command::RippleAffectedTracks, &["F"]),
    (Command::RippleWithMarkers, &["Ctrl+F"]),
    (Command::RippleAll, &["Ctrl+Shift+F"]),
    (Command::TrackMute, &["Z"]),
    (Command::TrackMuteExclusive, &["Shift+Z"]),
    (Command::TrackSolo, &["X"]),
    (Command::TrackSoloExclusive, &["Shift+X"]),
    (Command::TrackVolumeEnvelope, &["V"]),
    (Command::TrackPanEnvelope, &["P"]),
    (Command::TrackToggleEnvelopes, &["Ctrl+Shift+E"]),
    (Command::ViewFocusTimeline, &["Alt+0"]),
    (Command::ViewTrimmer, &["Alt+2"]),
    (Command::ViewMasterBus, &["Alt+3"]),
    (Command::ViewPreview, &["Alt+4"]),
    (Command::ViewProjectMedia, &["Alt+5"]),
    (Command::ViewLoudnessMeters, &["Ctrl+Alt+7"]),
    (Command::ViewVideoScopes, &["Ctrl+Alt+2"]),
    (Command::ViewAudioEffects, &["Ctrl+Alt+9"]),
];

/// The shortcuts of `command` in the default map.
pub fn default_shortcuts(command: Command) -> Vec<Shortcut> {
    DEFAULT_BINDINGS
        .iter()
        .find(|(c, _)| *c == command)
        .map(|(_, list)| {
            list.iter()
                .map(|s| s.parse().expect("default shortcut must parse"))
                .collect()
        })
        .unwrap_or_default()
}

/// Maps shortcuts to commands (one command per shortcut, several shortcuts per command).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMap {
    by_shortcut: BTreeMap<Shortcut, Command>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::defaults()
    }
}

impl KeyMap {
    /// The built-in map.
    pub fn defaults() -> Self {
        let mut by_shortcut = BTreeMap::new();
        for (command, list) in DEFAULT_BINDINGS {
            for text in *list {
                let shortcut: Shortcut = text.parse().expect("default shortcut must parse");
                by_shortcut.insert(shortcut, *command);
            }
        }
        Self { by_shortcut }
    }

    /// Applies the user's overrides on top of the defaults.
    ///
    /// Returns the map and a list of human-readable warnings (unknown commands, unreadable
    /// shortcuts, shortcuts taken away from another command). A listed command loses all of its
    /// default shortcuts and gets exactly the listed ones.
    pub fn from_overrides(overrides: &BTreeMap<String, Vec<String>>) -> (Self, Vec<String>) {
        let mut map = Self::defaults();
        let mut warnings = Vec::new();
        for (id, list) in overrides {
            let Some(command) = Command::from_id(id) else {
                warnings.push(format!("comando desconhecido «{id}» ignorado"));
                continue;
            };
            map.clear_command(command);
            for text in list {
                match text.parse::<Shortcut>() {
                    Ok(shortcut) => {
                        if let Some(displaced) = map.bind(command, shortcut.clone()) {
                            warnings.push(format!(
                                "{shortcut} saiu de «{}» e foi para «{}»",
                                displaced.label(),
                                command.label()
                            ));
                        }
                    }
                    Err(e) => warnings.push(format!("{id}: {e}")),
                }
            }
        }
        (map, warnings)
    }

    /// The command bound to `shortcut`.
    pub fn command_for(&self, shortcut: &Shortcut) -> Option<Command> {
        self.by_shortcut.get(shortcut).copied()
    }

    /// Every shortcut of `command`, in a stable order.
    pub fn shortcuts_for(&self, command: Command) -> Vec<Shortcut> {
        self.by_shortcut
            .iter()
            .filter(|(_, c)| **c == command)
            .map(|(s, _)| s.clone())
            .collect()
    }

    /// Binds `shortcut` to `command`. When another command owned it, that command loses the
    /// shortcut and is returned so the UI can tell the user.
    pub fn bind(&mut self, command: Command, shortcut: Shortcut) -> Option<Command> {
        match self.by_shortcut.insert(shortcut, command) {
            Some(previous) if previous != command => Some(previous),
            _ => None,
        }
    }

    pub fn unbind(&mut self, shortcut: &Shortcut) {
        self.by_shortcut.remove(shortcut);
    }

    /// Removes every shortcut of `command`.
    pub fn clear_command(&mut self, command: Command) {
        self.by_shortcut.retain(|_, c| *c != command);
    }

    /// Restores the default shortcuts of `command` (taking them from other commands if needed).
    pub fn reset_command(&mut self, command: Command) {
        self.clear_command(command);
        for shortcut in default_shortcuts(command) {
            self.by_shortcut.insert(shortcut, command);
        }
    }

    /// Bindings ordered for dispatch: most modifiers first, then by shortcut.
    pub fn dispatch_order(&self) -> Vec<(&Shortcut, Command)> {
        let mut list: Vec<_> = self.by_shortcut.iter().map(|(s, c)| (s, *c)).collect();
        list.sort_by_key(|(s, _)| std::cmp::Reverse(s.modifier_count()));
        list
    }

    /// The differences from the defaults, ready to store in the preferences.
    pub fn to_overrides(&self) -> BTreeMap<String, Vec<String>> {
        let mut out = BTreeMap::new();
        for &command in Command::ALL {
            let mut current: Vec<String> = self
                .shortcuts_for(command)
                .iter()
                .map(ToString::to_string)
                .collect();
            let mut default: Vec<String> = default_shortcuts(command)
                .iter()
                .map(ToString::to_string)
                .collect();
            current.sort();
            default.sort();
            if current != default {
                out.insert(command.id().to_string(), current);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn sc(text: &str) -> Shortcut {
        text.parse().unwrap()
    }

    #[test]
    fn parses_and_formats_canonical_names() {
        assert_eq!(sc("ctrl+shift+v").to_string(), "Ctrl+Shift+V");
        assert_eq!(sc("Shift+Alt+Ctrl+Left").to_string(), "Ctrl+Alt+Shift+Left");
        assert_eq!(sc("alt+[").to_string(), "Alt+[");
        assert_eq!(sc("f8").to_string(), "F8");
        assert_eq!(sc("Ctrl+-").to_string(), "Ctrl+-");
        assert_eq!(sc("esc").key, "Escape");
        assert_eq!(sc("Page Up").key, "PageUp");
    }

    #[test]
    fn rejects_bad_shortcuts() {
        for bad in ["", "Ctrl+", "Hyper+A", "Ctrl+Banana", "F13", "+"] {
            assert!(bad.parse::<Shortcut>().is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn display_then_parse_is_identity() {
        for (_, list) in DEFAULT_BINDINGS {
            for text in *list {
                let s = sc(text);
                assert_eq!(sc(&s.to_string()), s);
                assert_eq!(s.to_string(), *text, "default must be written canonically");
            }
        }
    }

    #[test]
    fn defaults_have_no_duplicate_shortcuts_or_commands() {
        let mut shortcuts = HashSet::new();
        let mut commands = HashSet::new();
        for (command, list) in DEFAULT_BINDINGS {
            assert!(commands.insert(*command), "{:?} listed twice", command);
            for text in *list {
                assert!(shortcuts.insert(sc(text)), "{text} bound twice");
            }
        }
        assert_eq!(KeyMap::defaults().by_shortcut.len(), shortcuts.len());
    }

    #[test]
    fn implemented_commands_all_have_a_default_shortcut() {
        for &command in Command::ALL {
            if command.implemented() {
                assert!(
                    !default_shortcuts(command).is_empty(),
                    "{} has no default shortcut",
                    command.id()
                );
            }
        }
    }

    #[test]
    fn current_shortcuts_are_preserved() {
        let map = KeyMap::defaults();
        let expect = [
            ("Ctrl+Q", Command::InsertAudioTrack),
            ("Ctrl+Shift+Q", Command::InsertVideoTrack),
            ("S", Command::EditSplit),
            ("Space", Command::TransportPlayToggle),
            ("Left", Command::CursorLeftByFrame),
            ("Right", Command::CursorRightByFrame),
            ("Delete", Command::EditDelete),
            ("F8", Command::OptionsSnapping),
            ("Ctrl+Z", Command::EditUndo),
            ("Ctrl+Y", Command::EditRedo),
            ("Ctrl+Shift+Z", Command::EditRedo),
            ("Ctrl+O", Command::FileOpen),
            ("Ctrl+S", Command::FileSave),
            ("Ctrl+Shift+S", Command::FileSaveAs),
        ];
        for (text, command) in expect {
            assert_eq!(map.command_for(&sc(text)), Some(command), "{text}");
        }
    }

    #[test]
    fn dispatch_order_puts_more_modifiers_first() {
        let map = KeyMap::defaults();
        let order = map.dispatch_order();
        let counts: Vec<u8> = order.iter().map(|(s, _)| s.modifier_count()).collect();
        assert!(counts.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn binding_a_taken_shortcut_displaces_the_other_command() {
        let mut map = KeyMap::defaults();
        let displaced = map.bind(Command::EditUndo, sc("S"));
        assert_eq!(displaced, Some(Command::EditSplit));
        assert_eq!(map.command_for(&sc("S")), Some(Command::EditUndo));
        assert!(map.shortcuts_for(Command::EditSplit).is_empty());
        // Rebinding the same shortcut to the same command displaces nobody
        assert_eq!(map.bind(Command::EditUndo, sc("S")), None);
    }

    #[test]
    fn reset_restores_defaults() {
        let mut map = KeyMap::defaults();
        map.clear_command(Command::EditSplit);
        map.bind(Command::EditSplit, sc("F10"));
        map.reset_command(Command::EditSplit);
        assert_eq!(map.shortcuts_for(Command::EditSplit), vec![sc("S")]);
        assert_eq!(map.to_overrides(), BTreeMap::new());
    }

    #[test]
    fn overrides_round_trip_and_only_store_differences() {
        let mut map = KeyMap::defaults();
        map.clear_command(Command::EditSplit);
        map.bind(Command::EditSplit, sc("F10"));
        map.bind(Command::EditSplit, sc("Ctrl+K"));
        map.unbind(&sc("Alt+Backspace"));
        let overrides = map.to_overrides();
        assert_eq!(overrides.len(), 2);
        assert_eq!(overrides["edit.split"], vec!["Ctrl+K", "F10"]);
        assert_eq!(overrides["edit.undo"], vec!["Ctrl+Z"]);

        let (rebuilt, warnings) = KeyMap::from_overrides(&overrides);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(rebuilt, map);
    }

    #[test]
    fn empty_override_unbinds_a_command() {
        let overrides = BTreeMap::from([("edit.split".to_string(), Vec::new())]);
        let (map, warnings) = KeyMap::from_overrides(&overrides);
        assert!(warnings.is_empty());
        assert_eq!(map.command_for(&sc("S")), None);
        assert!(map.shortcuts_for(Command::EditSplit).is_empty());
    }

    #[test]
    fn bad_overrides_are_reported_but_never_panic() {
        let overrides = BTreeMap::from([
            ("no.such.command".to_string(), vec!["Ctrl+Q".to_string()]),
            (
                "edit.split".to_string(),
                vec!["Banana".to_string(), "F10".to_string()],
            ),
            // Takes Ctrl+Q from "insert audio track"
            ("edit.undo".to_string(), vec!["Ctrl+Q".to_string()]),
        ]);
        let (map, warnings) = KeyMap::from_overrides(&overrides);
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert_eq!(map.command_for(&sc("F10")), Some(Command::EditSplit));
        assert_eq!(map.command_for(&sc("Ctrl+Q")), Some(Command::EditUndo));
        assert!(map.shortcuts_for(Command::InsertAudioTrack).is_empty());
    }
}
