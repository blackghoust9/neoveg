//! A scan of the source for hover tips that never go through `tr`.
//!
//! A tip written as `on_hover_text("texto")` shows Portuguese in every language, and nothing at run
//! time notices (the interface scan reads only what is on screen, and a tip is not until the mouse
//! rests on it). This test reads the sources and lists every tip whose text is a plain literal; the
//! fix is `on_hover_text(tr("texto"))` with the text added to the catalogs.

use std::path::{Path, PathBuf};

/// The calls that put a text in a tip.
const CALLS: [&str; 3] = [
    ".on_hover_text(",
    ".on_disabled_hover_text(",
    ".on_hover_text_at_pointer(",
];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The argument of the call that starts at `from` looks like text shown as it is: a string
/// literal, or `format!` of a literal that has words of its own (not only `{}` and punctuation).
fn is_plain_text(argument: &str) -> bool {
    let argument = argument.trim_start();
    let literal = argument
        .strip_prefix("format!(")
        .map_or(argument, str::trim_start);
    let Some(body) = literal.strip_prefix('"') else {
        return false;
    };
    // `{}` and the like are values and a backslash escape (a line break) is not a word: only
    // letters outside both count
    let mut chars = body.chars();
    let mut in_braces = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => break,
            '\\' => {
                chars.next();
            }
            '{' => in_braces = true,
            '}' => in_braces = false,
            c if !in_braces && c.is_alphabetic() => return true,
            _ => {}
        }
    }
    false
}

/// `file:line` of every tip with a plain literal in `source`.
pub fn untranslated_tips(source: &str) -> Vec<usize> {
    let mut found = Vec::new();
    for call in CALLS {
        for (at, _) in source.match_indices(call) {
            let after = &source[at + call.len()..];
            if is_plain_text(after) {
                found.push(source[..at].matches('\n').count() + 1);
            }
        }
    }
    found.sort_unstable();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tip_that_is_a_plain_literal_is_found_and_one_through_tr_is_not() {
        let source = r#"
            ui.button("a").on_hover_text("Salvar o projeto");
            ui.button("b").on_hover_text(tr("Salvar o projeto"));
            ui.button("c").on_hover_text(
                "Abrir"
            );
            ui.button("d").on_hover_text(format!("Pasta: {}", path));
            ui.button("e").on_hover_text(format!("{}\n{}", a, b));
            ui.button("f").on_hover_text(&entry.path);
            ui.button("g").on_disabled_hover_text("Sem faixa");
        "#;
        assert_eq!(untranslated_tips(source), vec![2, 4, 7, 10]);
    }

    /// Every tip of the interface goes through `tr`. A new tip with a literal text fails here.
    #[test]
    fn no_tip_of_the_interface_skips_the_translation() {
        let mut files = Vec::new();
        sources(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        assert!(files.len() > 20, "the sources were not found");
        let mut bad = Vec::new();
        for file in files {
            // This file shows the pattern in its own tests
            if file.file_name().is_some_and(|n| n == "tips_audit.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            for line in untranslated_tips(&text) {
                bad.push(format!("{}:{line}", file.display()));
            }
        }
        assert!(bad.is_empty(), "tips that skip `tr`:\n{}", bad.join("\n"));
    }
}
