//! Templates for command markers (manual v21, "Using command templates"): a name that stands for
//! a command, a parameter and a comment, so a command that is inserted often is filled in with one
//! choice. They are kept in the preferences, for every project.

use serde::{Deserialize, Serialize};

/// Most templates kept.
pub const MAX_TEMPLATES: usize = 64;
/// Longest name of a template, in characters.
pub const MAX_NAME: usize = 60;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CommandTemplate {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub parameter: String,
    #[serde(default)]
    pub comment: String,
}

/// The name as it is kept: trimmed and at most [`MAX_NAME`] characters; `None` when nothing is left.
pub fn clean_name(name: &str) -> Option<String> {
    let name: String = name.trim().chars().take(MAX_NAME).collect();
    (!name.is_empty()).then_some(name)
}

/// Saves `template` in `list`: a template with the same name (ignoring case) is replaced, a new one
/// goes to the end. `false` when the name or the command is empty, or the list is full.
pub fn save(list: &mut Vec<CommandTemplate>, template: &CommandTemplate) -> bool {
    let (Some(name), false) = (
        clean_name(&template.name),
        template.command.trim().is_empty(),
    ) else {
        return false;
    };
    let kept = CommandTemplate {
        name: name.clone(),
        command: template.command.trim().to_owned(),
        parameter: template.parameter.trim().to_owned(),
        comment: template.comment.trim().to_owned(),
    };
    let same = list
        .iter()
        .position(|t| t.name.to_lowercase() == name.to_lowercase());
    match same {
        Some(index) => list[index] = kept,
        None if list.len() >= MAX_TEMPLATES => return false,
        None => list.push(kept),
    }
    true
}

/// Deletes the template called `name`; `true` when there was one.
pub fn remove(list: &mut Vec<CommandTemplate>, name: &str) -> bool {
    let before = list.len();
    list.retain(|t| t.name != name);
    list.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(name: &str, command: &str) -> CommandTemplate {
        CommandTemplate {
            name: name.into(),
            command: command.into(),
            parameter: " https://example.org ".into(),
            comment: String::new(),
        }
    }

    #[test]
    fn a_template_is_saved_replaced_by_name_and_removed() {
        let mut list = Vec::new();
        assert!(save(&mut list, &template("Link", "URL")));
        assert_eq!(list[0].parameter, "https://example.org", "trimmed");
        assert!(save(&mut list, &template("Legenda", "Text")));
        // The same name again (any case) replaces it
        assert!(save(&mut list, &template("link", "Headline")));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].command, "Headline");
        assert!(remove(&mut list, "Legenda"));
        assert!(!remove(&mut list, "Legenda"));
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn a_template_needs_a_name_and_a_command_and_the_list_has_a_limit() {
        let mut list = Vec::new();
        assert!(!save(&mut list, &template("  ", "URL")));
        assert!(!save(&mut list, &template("x", "  ")));
        assert!(list.is_empty());
        for n in 0..MAX_TEMPLATES {
            assert!(save(&mut list, &template(&format!("t{n}"), "URL")));
        }
        assert!(!save(&mut list, &template("one more", "URL")));
        // Replacing one still works when the list is full
        assert!(save(&mut list, &template("t0", "Text")));
        assert_eq!(
            clean_name(&"y".repeat(200)).unwrap().chars().count(),
            MAX_NAME
        );
        let json = serde_json::to_string(&list[0]).unwrap();
        assert_eq!(
            serde_json::from_str::<CommandTemplate>(&json).unwrap(),
            list[0]
        );
    }
}
