//! Presets of an effect (manual v21, "Using presets"): a name that stands for the values of every
//! parameter of one effect, so a look that is used often is applied with one choice. Like the
//! presets of the plug-ins of the reference, a preset belongs to one effect (`kind`) and is shared
//! by every project; it can be exported to a file and imported from one.
//!
//! The effect registries live in other crates, so a preset only holds the registry id and the
//! numbers; the panel that applies it clamps the values to the ranges of the effect.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Most presets kept, over every effect.
pub const MAX_PRESETS: usize = 512;
/// Longest name of a preset, in characters.
pub const MAX_NAME: usize = 60;
/// Most parameters in one preset.
pub const MAX_PARAMS: usize = 64;
/// Longest registry id or parameter id, in characters.
pub const MAX_ID: usize = 64;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EffectPreset {
    /// Registry id of the effect the preset is for.
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub params: BTreeMap<String, f32>,
}

/// What a file of presets looks like on disk.
#[derive(Serialize, Deserialize)]
struct PresetFile {
    #[serde(default)]
    presets: Vec<EffectPreset>,
}

/// The name as it is kept: trimmed and at most [`MAX_NAME`] characters; `None` when nothing is left.
pub fn clean_name(name: &str) -> Option<String> {
    let name: String = name.trim().chars().take(MAX_NAME).collect();
    (!name.is_empty()).then_some(name)
}

/// The preset made safe: a name and an effect id that fit, no more than [`MAX_PARAMS`] numbers and
/// only finite ones. `None` when no name or effect is left.
pub fn sanitized(preset: &EffectPreset) -> Option<EffectPreset> {
    let name = clean_name(&preset.name)?;
    let kind: String = preset.kind.trim().chars().take(MAX_ID).collect();
    if kind.is_empty() {
        return None;
    }
    let params = preset
        .params
        .iter()
        .filter(|(id, value)| !id.is_empty() && id.chars().count() <= MAX_ID && value.is_finite())
        .take(MAX_PARAMS)
        .map(|(id, value)| (id.clone(), *value))
        .collect();
    Some(EffectPreset { kind, name, params })
}

fn same(a: &EffectPreset, b: &EffectPreset) -> bool {
    a.kind == b.kind && a.name.to_lowercase() == b.name.to_lowercase()
}

/// Saves `preset` in `list`: one with the same effect and name (ignoring case) is replaced, a new
/// one goes to the end. `false` when it has no name or effect, or the list is full.
pub fn save(list: &mut Vec<EffectPreset>, preset: &EffectPreset) -> bool {
    let Some(kept) = sanitized(preset) else {
        return false;
    };
    match list.iter().position(|p| same(p, &kept)) {
        Some(index) => list[index] = kept,
        None if list.len() >= MAX_PRESETS => return false,
        None => list.push(kept),
    }
    true
}

/// Deletes the preset `name` of the effect `kind`; `true` when there was one.
pub fn remove(list: &mut Vec<EffectPreset>, kind: &str, name: &str) -> bool {
    let before = list.len();
    list.retain(|p| !(p.kind == kind && p.name == name));
    list.len() != before
}

/// The presets of one effect, in the order they were saved.
pub fn for_kind<'a>(
    list: &'a [EffectPreset],
    kind: &'a str,
) -> impl Iterator<Item = &'a EffectPreset> {
    list.iter().filter(move |p| p.kind == kind)
}

/// The presets of the effect `kind` (all of them when `None`) as the text of a file.
pub fn to_json(list: &[EffectPreset], kind: Option<&str>) -> String {
    let presets = list
        .iter()
        .filter(|p| kind.is_none_or(|k| p.kind == k))
        .cloned()
        .collect();
    serde_json::to_string_pretty(&PresetFile { presets }).unwrap_or_default()
}

/// Reads the text of a file of presets. A text that is not one is an error with the reason; the
/// presets in it are made safe and the ones that cannot be are left out.
pub fn from_json(text: &str) -> Result<Vec<EffectPreset>, String> {
    let file: PresetFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
    Ok(file.presets.iter().filter_map(sanitized).collect())
}

/// Puts the `incoming` presets in `list` (see [`save`]); returns how many were kept.
pub fn merge(list: &mut Vec<EffectPreset>, incoming: &[EffectPreset]) -> usize {
    incoming.iter().filter(|p| save(list, p)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(kind: &str, name: &str, value: f32) -> EffectPreset {
        EffectPreset {
            kind: kind.into(),
            name: name.into(),
            params: BTreeMap::from([("amount".to_owned(), value)]),
        }
    }

    #[test]
    fn a_preset_is_saved_replaced_by_effect_and_name_and_removed() {
        let mut list = Vec::new();
        assert!(save(&mut list, &preset("blur", "Suave", 0.2)));
        assert!(save(&mut list, &preset("sepia", "Suave", 0.5)));
        assert_eq!(
            list.len(),
            2,
            "the same name on another effect is another preset"
        );
        assert!(save(&mut list, &preset("blur", "  suave ", 0.9)));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].params["amount"], 0.9);
        assert_eq!(for_kind(&list, "blur").count(), 1);
        assert!(remove(&mut list, "blur", "suave"));
        assert!(!remove(&mut list, "blur", "suave"), "the name is exact");
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn a_preset_needs_a_name_and_an_effect_and_odd_numbers_are_dropped() {
        let mut list = Vec::new();
        assert!(!save(&mut list, &preset("blur", "  ", 1.0)));
        assert!(!save(&mut list, &preset("  ", "x", 1.0)));
        let mut odd = preset("blur", "x", 1.0);
        odd.params.insert("nan".into(), f32::NAN);
        odd.params.insert("inf".into(), f32::INFINITY);
        odd.params.insert(String::new(), 1.0);
        let kept = sanitized(&odd).unwrap();
        assert_eq!(kept.params.len(), 1);
        for n in 0..MAX_PRESETS {
            assert!(save(&mut list, &preset("blur", &format!("p{n}"), 0.0)));
        }
        assert!(!save(&mut list, &preset("blur", "one more", 0.0)));
        assert!(
            save(&mut list, &preset("blur", "p0", 1.0)),
            "replacing still works"
        );
    }

    #[test]
    fn presets_go_to_a_file_and_come_back_and_a_wrong_file_is_refused() {
        let list = vec![preset("blur", "A", 0.1), preset("sepia", "B", 0.2)];
        let text = to_json(&list, Some("blur"));
        let back = from_json(&text).unwrap();
        assert_eq!(back, vec![list[0].clone()]);
        assert_eq!(from_json(&to_json(&list, None)).unwrap(), list);
        assert!(from_json("não é json").is_err());
        // A preset without a name is left out, the others are kept
        let mixed = r#"{"presets":[{"kind":"blur","name":""},{"kind":"blur","name":"ok"}]}"#;
        assert_eq!(from_json(mixed).unwrap().len(), 1);
        let mut mine = vec![preset("blur", "ok", 0.0)];
        assert_eq!(merge(&mut mine, &back), 1);
        assert_eq!(mine.len(), 2);
    }
}
