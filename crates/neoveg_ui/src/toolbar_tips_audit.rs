//! The toolbar tooltips come from the single action table (`Command`), with the shortcut of the
//! keyboard map, instead of text written next to each button.

/// The source of `toolbar_ui` (up to the next function).
fn toolbar_source() -> String {
    let src = include_str!("app.rs");
    let from = src.find("fn toolbar_ui(").expect("toolbar_ui exists");
    let rest = &src[from..];
    let to = rest.find("fn timeline_canvas(").expect("timeline_canvas follows");
    rest[..to].to_string()
}

/// Whether `source` writes a tooltip as a literal passed to `tr("...")`.
fn literal_tips(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find("tr(\"") {
        let after = &rest[at + 4..];
        let end = after.find('"').unwrap_or(after.len());
        found.push(after[..end].to_string());
        rest = &after[end..];
    }
    found
}

#[test]
fn toolbar_tips_come_from_the_command_table() {
    let tips = literal_tips(&toolbar_source());
    assert!(
        tips.is_empty(),
        "toolbar buttons must take their tip from `Command::label` + `with_hint`: {tips:?}"
    );
}

#[test]
fn the_scan_sees_a_literal_tip() {
    assert_eq!(
        literal_tips("icon_button(ui, I, tr(\"Abc\"), false); tr(\"Def\")"),
        vec!["Abc".to_string(), "Def".to_string()]
    );
}
