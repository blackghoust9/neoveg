//! Title templates with replaceable text (live text): a title whose text holds named fields
//! written `{{name}}`. The look (font, colours, stack of fills and outlines, animation) is kept
//! as it is and the fields are filled in when the template is used.
//!
//! The styled stretches of the text (`TitleSpan`) are counted in characters of the template text,
//! so filling a field moves them to follow the new text.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::generators::{GeneratorKind, GeneratorSpec};

/// File extension of a title template saved on disk.
pub const TEMPLATE_EXTENSION: &str = "neovegtitulo";

/// One replaceable field of a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateField {
    pub name: String,
    /// What the field says while the template is not filled in.
    pub default: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleTemplate {
    pub name: String,
    /// A title generator whose text holds the `{{field}}` markers.
    pub spec: GeneratorSpec,
    /// Defaults of the fields; a field not listed here starts empty.
    #[serde(default)]
    pub defaults: BTreeMap<String, String>,
}

/// A piece of the template text: plain text or a field.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Text(String),
    Field(String),
}

fn parse(text: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open + 2..].find("}}") else {
            break;
        };
        let name = rest[open + 2..open + 2 + close].trim();
        if name.is_empty() || name.contains('{') || name.contains('\n') {
            // Not a field: keep the braces as text and go on after them
            pieces.push(Piece::Text(rest[..open + 2].to_string()));
            rest = &rest[open + 2..];
            continue;
        }
        if open > 0 {
            pieces.push(Piece::Text(rest[..open].to_string()));
        }
        pieces.push(Piece::Field(name.to_string()));
        rest = &rest[open + 2 + close + 2..];
    }
    if !rest.is_empty() {
        pieces.push(Piece::Text(rest.to_string()));
    }
    pieces
}

/// The names of the fields of `text`, in order of appearance, each once.
pub fn field_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for piece in parse(text) {
        if let Piece::Field(n) = piece {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names
}

/// Fills the fields of `text` with `values` (a field with no value becomes empty). Returns the new
/// text and, for every character index `0..=chars` of the old text, the index it moves to. A
/// position inside a marker maps to the start of its replacement, the end of a marker to the end.
pub fn fill_text(text: &str, values: &BTreeMap<String, String>) -> (String, Vec<usize>) {
    let mut out = String::new();
    let mut out_chars = 0usize;
    let mut map: Vec<usize> = Vec::new();
    for piece in parse(text) {
        match piece {
            Piece::Text(t) => {
                for c in t.chars() {
                    map.push(out_chars);
                    out.push(c);
                    out_chars += 1;
                }
            }
            Piece::Field(name) => {
                // Every character of the marker: `{{`, the name with its spaces, `}}`
                let marker = marker_len(text, &map);
                let value = values.get(&name).map(String::as_str).unwrap_or("");
                let value_chars = value.chars().count();
                for _ in 0..marker {
                    map.push(out_chars);
                }
                out.push_str(value);
                out_chars += value_chars;
            }
        }
    }
    map.push(out_chars);
    (out, map)
}

/// Length in characters of the marker that starts at the character `map.len()` of `text`.
fn marker_len(text: &str, map: &[usize]) -> usize {
    let tail: String = text.chars().skip(map.len()).collect();
    match tail.find("}}") {
        Some(end) => tail[..end + 2].chars().count(),
        None => tail.chars().count(),
    }
}

impl TitleTemplate {
    pub fn new(name: &str, spec: GeneratorSpec) -> Self {
        Self {
            name: name.to_string(),
            spec,
            defaults: BTreeMap::new(),
        }
    }

    fn template_text(&self) -> &str {
        match &self.spec.kind {
            GeneratorKind::TitleText { text, .. } => text,
            _ => "",
        }
    }

    /// The fields of the template with their defaults.
    pub fn fields(&self) -> Vec<TemplateField> {
        field_names(self.template_text())
            .into_iter()
            .map(|name| TemplateField {
                default: self.defaults.get(&name).cloned().unwrap_or_default(),
                name,
            })
            .collect()
    }

    /// The title with the fields filled in: `values` first, the defaults for the fields it leaves
    /// out. Styled stretches follow their text.
    pub fn instantiate(&self, values: &BTreeMap<String, String>) -> GeneratorSpec {
        let mut all = self.defaults.clone();
        all.extend(values.iter().map(|(k, v)| (k.clone(), v.clone())));
        let mut spec = self.spec.clone();
        if let GeneratorKind::TitleText { text, layout, .. } = &mut spec.kind {
            let (filled, map) = fill_text(text, &all);
            let at = |i: usize| {
                map.get(i)
                    .copied()
                    .unwrap_or_else(|| map.last().copied().unwrap_or(0))
            };
            for span in &mut layout.spans {
                span.start = at(span.start);
                span.end = at(span.end);
            }
            layout.spans.retain(|s| s.end > s.start);
            *text = filled;
        }
        spec
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("falha ao serializar o modelo de título: {e}"))?;
        std::fs::write(path, json)
            .map_err(|e| format!("falha ao gravar o modelo de título {}: {e}", path.display()))
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("falha ao ler o modelo de título {}: {e}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("modelo de título inválido {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::TitleSpan;

    fn template(text: &str) -> TitleTemplate {
        let mut spec = GeneratorSpec {
            name: "Terço inferior".into(),
            width: 320,
            height: 180,
            fps: 25.0,
            duration: 3.0,
            kind: GeneratorKind::default_title(),
        };
        if let GeneratorKind::TitleText { text: t, .. } = &mut spec.kind {
            *t = text.into();
        }
        TitleTemplate::new("Terço inferior", spec)
    }

    fn text_of(spec: &GeneratorSpec) -> String {
        match &spec.kind {
            GeneratorKind::TitleText { text, .. } => text.clone(),
            _ => String::new(),
        }
    }

    fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn the_fields_are_found_in_order_without_repeats() {
        assert_eq!(
            field_names("{{nome}} - {{ cargo }} ({{nome}}) {{}} {x}"),
            vec!["nome".to_string(), "cargo".to_string()]
        );
        assert!(field_names("sem campos {{ aberto").is_empty());
        let t = template("{{nome}}\n{{cargo}}");
        let fields = t.fields();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].name, "cargo");
    }

    #[test]
    fn filling_replaces_the_fields_and_keeps_the_look() {
        let mut t = template("{{nome}}\n{{cargo}}");
        t.defaults.insert("cargo".into(), "Repórter".into());
        let spec = t.instantiate(&values(&[("nome", "João da Silva")]));
        assert_eq!(text_of(&spec), "João da Silva\nRepórter");
        // Same look as the template
        assert_eq!(spec.width, t.spec.width);
        let other = t.instantiate(&values(&[("nome", "Ana"), ("cargo", "Chefe")]));
        assert_eq!(text_of(&other), "Ana\nChefe");
        // The template itself is left alone and shows its markers
        assert_eq!(text_of(&t.spec), "{{nome}}\n{{cargo}}");
        // A missing field turns into nothing
        assert_eq!(
            text_of(&template("A {{x}} B").instantiate(&values(&[]))),
            "A  B"
        );
    }

    #[test]
    fn styled_stretches_follow_their_text() {
        // "Olá {{nome}}!": the span covers the characters of the marker (4..12) and the "!"
        let mut t = template("Olá {{nome}}!");
        if let GeneratorKind::TitleText { layout, .. } = &mut t.spec.kind {
            let mut bold = TitleSpan::new(4, 12);
            bold.bold = true;
            layout.spans = vec![bold, TitleSpan::new(12, 13)];
        }
        let spec = t.instantiate(&values(&[("nome", "Maria Clara")]));
        let GeneratorKind::TitleText { text, layout, .. } = &spec.kind else {
            panic!("a title")
        };
        assert_eq!(text, "Olá Maria Clara!");
        let chars: Vec<char> = text.chars().collect();
        let covered: String = chars[layout.spans[0].start..layout.spans[0].end]
            .iter()
            .collect();
        assert_eq!(covered, "Maria Clara");
        let tail: String = chars[layout.spans[1].start..layout.spans[1].end]
            .iter()
            .collect();
        assert_eq!(tail, "!");
    }

    #[test]
    fn a_template_is_saved_and_loaded_and_renders_the_filled_text() {
        let dir = std::env::temp_dir().join(format!("neoveg_tpl_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("modelo.{TEMPLATE_EXTENSION}"));
        let mut t = template("{{nome}}");
        t.defaults.insert("nome".into(), "NOME".into());
        t.save(&path).unwrap();
        let back = TitleTemplate::load(&path).unwrap();
        assert_eq!(back, t);
        let one = back.instantiate(&values(&[("nome", "ABC")])).render(0.0, 0);
        let two = back
            .instantiate(&values(&[("nome", "ABCDEFGH")]))
            .render(0.0, 0);
        assert_ne!(one.rgba, two.rgba, "another text, another picture");
        assert!(TitleTemplate::load(&dir.join("nao_existe")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
