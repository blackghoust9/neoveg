//! The buttons of the main toolbar and their order (*Exibir > Personalizar barra de ferramentas*),
//! the way the reference lets the user add, remove and reorder the buttons of a toolbar and go
//! back to the standard set.
//!
//! The toolbar is a list of text items kept in the preferences: the dotted id of a command (the
//! same ids as the keyboard map), one of the three program buttons ([`RENDER`], [`WELCOME`],
//! [`PREFERENCES`]) or [`SEPARATOR`]. Which icon a command has is up to the interface; here only
//! the list is kept sound.

use crate::commands::Command;

pub const SEPARATOR: &str = "|";
/// The program buttons that are not commands.
pub const RENDER: &str = "app.render";
pub const WELCOME: &str = "app.welcome";
pub const PREFERENCES: &str = "app.preferences";
pub const APP_ITEMS: [&str; 3] = [RENDER, WELCOME, PREFERENCES];
/// Most items in the toolbar.
pub const MAX_ITEMS: usize = 64;

/// The toolbar before the user changes it.
pub fn default_items() -> Vec<String> {
    let commands = |list: &[Command]| list.iter().map(|c| c.id().to_owned()).collect::<Vec<_>>();
    let mut items = commands(&[
        Command::FileNew,
        Command::FileOpen,
        Command::FileSave,
        Command::FileProperties,
    ]);
    items.push(RENDER.to_owned());
    items.push(SEPARATOR.to_owned());
    items.extend(commands(&[
        Command::EditCut,
        Command::EditCopy,
        Command::EditPaste,
        Command::EditDelete,
    ]));
    items.push(SEPARATOR.to_owned());
    items.extend(commands(&[Command::EditUndo, Command::EditRedo]));
    items.push(SEPARATOR.to_owned());
    items.push(WELCOME.to_owned());
    items.push(PREFERENCES.to_owned());
    items
}

/// Whether `id` is a separator, a program button or a command that exists.
pub fn is_known(id: &str) -> bool {
    id == SEPARATOR || APP_ITEMS.contains(&id) || Command::from_id(id).is_some()
}

/// The list made sound: unknown items and repeated buttons dropped, no separator at an end or
/// next to another, at most [`MAX_ITEMS`].
pub fn sanitized(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if !is_known(item) {
            continue;
        }
        if item == SEPARATOR {
            if out.last().is_none_or(|last| last == SEPARATOR) {
                continue;
            }
        } else if out.contains(item) {
            continue;
        }
        out.push(item.clone());
    }
    while out.last().is_some_and(|last| last == SEPARATOR) {
        out.pop();
    }
    out.truncate(MAX_ITEMS);
    out
}

/// Moves the item at `index` one place up (`up`) or down; `false` at the ends.
pub fn move_item(items: &mut [String], index: usize, up: bool) -> bool {
    let other = if up {
        index.checked_sub(1)
    } else {
        index.checked_add(1)
    };
    match other {
        Some(other) if index < items.len() && other < items.len() => {
            items.swap(index, other);
            true
        }
        _ => false,
    }
}

/// Takes the item at `index` out; `false` when there is none.
pub fn remove_item(items: &mut Vec<String>, index: usize) -> bool {
    if index < items.len() {
        items.remove(index);
        true
    } else {
        false
    }
}

/// Puts `id` at the end: a separator any time, a button once. `false` when it is unknown, already
/// there, or the toolbar is full.
pub fn add_item(items: &mut Vec<String>, id: &str) -> bool {
    if !is_known(id)
        || items.len() >= MAX_ITEMS
        || (id != SEPARATOR && items.iter().any(|i| i == id))
    {
        return false;
    }
    items.push(id.to_owned());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_default_toolbar_is_sound_and_has_every_button_once() {
        let items = default_items();
        assert_eq!(sanitized(&items), items, "nothing in it needs fixing");
        assert!(items.contains(&Command::EditUndo.id().to_owned()));
        assert!(items.contains(&RENDER.to_owned()));
        assert_eq!(
            items.first().map(String::as_str),
            Some(Command::FileNew.id())
        );
    }

    #[test]
    fn a_hand_edited_list_is_made_sound() {
        let messy = list(&[
            "|",
            "file.new",
            "|",
            "|",
            "no.such.command",
            "file.new",
            "edit.cut",
            "app.render",
            "|",
        ]);
        assert_eq!(
            sanitized(&messy),
            list(&["file.new", "|", "edit.cut", "app.render"])
        );
        let long: Vec<String> = std::iter::repeat_n(["file.new", "|"], MAX_ITEMS)
            .flatten()
            .map(str::to_owned)
            .collect();
        assert!(sanitized(&long).len() <= MAX_ITEMS);
        assert!(sanitized(&[]).is_empty());
    }

    #[test]
    fn items_are_added_once_moved_and_removed() {
        let mut items = list(&["file.new", "edit.cut"]);
        assert!(add_item(&mut items, "edit.copy"));
        assert!(!add_item(&mut items, "edit.copy"), "a button once");
        assert!(add_item(&mut items, SEPARATOR) && add_item(&mut items, SEPARATOR));
        assert!(!add_item(&mut items, "no.such"));
        assert!(move_item(&mut items, 2, true));
        assert_eq!(items[..3], list(&["file.new", "edit.copy", "edit.cut"])[..]);
        assert!(!move_item(&mut items, 0, true), "the top stays");
        let last = items.len() - 1;
        assert!(!move_item(&mut items, last, false), "the bottom stays");
        assert!(remove_item(&mut items, 1));
        assert!(!remove_item(&mut items, 99));
        assert_eq!(items[0], "file.new");
    }

    #[test]
    fn every_program_button_and_command_id_is_known() {
        for id in APP_ITEMS {
            assert!(is_known(id));
        }
        for command in Command::ALL {
            assert!(is_known(command.id()), "{}", command.id());
        }
        assert!(!is_known(""));
    }
}
