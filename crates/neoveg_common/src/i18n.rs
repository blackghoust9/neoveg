//! Interface language: the text in the code is Brazilian Portuguese (the default), and a catalog per
//! language maps that text to its translation.
//!
//! `tr("Arquivo")` gives the translation of "Arquivo" in the current language and the text itself
//! when the language is Portuguese, has no catalog, or the catalog lacks the entry. So an untranslated
//! label is shown in Portuguese, never blank and never a key, and the code can be converted one label
//! at a time.
//!
//! The catalogs are `assets/i18n/strings/<tag>.json` (a JSON object, Portuguese text to translation),
//! embedded in the program so a portable folder cannot lose them. A catalog is parsed once per
//! language and kept for the life of the program, which lets [`tr`] hand out `&'static str` without
//! allocating on every frame.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, RwLock};

/// A parsed catalog: Portuguese text to translation.
type Catalog = HashMap<&'static str, &'static str>;

/// The languages the interface offers (`Assets/i18n/*.json`), default first.
pub const LANGUAGES: [&str; 8] = [
    "pt-BR", "en-US", "de-DE", "es-ES", "fr-FR", "zh-CN", "ja-JP", "ru-RU",
];

/// The embedded catalogs. Portuguese is the source text and has none; Japanese and Chinese wait for a
/// font with those glyphs (the default egui fonts draw them as empty boxes).
const EMBEDDED: [(&str, &str); 5] = [
    (
        "en-US",
        include_str!("../../../assets/i18n/strings/en-US.json"),
    ),
    (
        "de-DE",
        include_str!("../../../assets/i18n/strings/de-DE.json"),
    ),
    (
        "es-ES",
        include_str!("../../../assets/i18n/strings/es-ES.json"),
    ),
    (
        "fr-FR",
        include_str!("../../../assets/i18n/strings/fr-FR.json"),
    ),
    (
        "ru-RU",
        include_str!("../../../assets/i18n/strings/ru-RU.json"),
    ),
];

static EMPTY: OnceLock<Catalog> = OnceLock::new();
static LOADED: Mutex<Option<HashMap<&'static str, &'static Catalog>>> = Mutex::new(None);
static CURRENT: RwLock<Option<(&'static str, &'static Catalog)>> = RwLock::new(None);

fn empty() -> &'static Catalog {
    EMPTY.get_or_init(HashMap::new)
}

/// Parses the embedded text of `tag` into a leaked catalog. A broken file gives an empty catalog (the
/// interface stays in Portuguese) and is reported by the tests, never by a panic at run time.
fn parse(text: &str) -> Catalog {
    let Ok(map) = serde_json::from_str::<HashMap<String, String>>(text) else {
        return HashMap::new();
    };
    map.into_iter()
        .filter(|(from, to)| !to.is_empty() && from != to)
        .map(|(from, to)| {
            let from: &'static str = Box::leak(from.into_boxed_str());
            let to: &'static str = Box::leak(to.into_boxed_str());
            (from, to)
        })
        .collect()
}

/// The language tag in [`LANGUAGES`] equal to `tag` (ignoring case), or Portuguese.
fn canonical(tag: &str) -> &'static str {
    LANGUAGES
        .iter()
        .copied()
        .find(|known| known.eq_ignore_ascii_case(tag.trim()))
        .unwrap_or(LANGUAGES[0])
}

fn catalog_for(tag: &'static str) -> &'static Catalog {
    let Some((_, text)) = EMBEDDED.iter().find(|(known, _)| *known == tag) else {
        return empty();
    };
    let mut loaded = LOADED.lock().unwrap_or_else(|e| e.into_inner());
    let loaded = loaded.get_or_insert_with(HashMap::new);
    loaded
        .entry(tag)
        .or_insert_with(|| Box::leak(Box::new(parse(text))))
}

/// Makes `tag` the interface language. An unknown tag selects Portuguese. Cheap when the language does
/// not change, so the application can call it every frame with the preference.
pub fn set_language(tag: &str) {
    let tag = canonical(tag);
    if language() == tag {
        return;
    }
    let catalog = catalog_for(tag);
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = Some((tag, catalog));
}

/// The tag of the interface language now in use.
pub fn language() -> &'static str {
    CURRENT
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .map_or(LANGUAGES[0], |(tag, _)| tag)
}

/// `text` in the interface language, or `text` itself when there is no translation.
pub fn tr(text: &'static str) -> &'static str {
    CURRENT
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .and_then(|(_, catalog)| catalog.get(text).copied())
        .unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_catalog_is_valid_json_with_text_for_text_entries() {
        for (tag, text) in EMBEDDED {
            let map: HashMap<String, String> = serde_json::from_str(text)
                .unwrap_or_else(|e| panic!("{tag}: catálogo inválido: {e}"));
            assert!(!map.is_empty(), "{tag}: catálogo vazio");
            for (from, to) in &map {
                assert!(!from.trim().is_empty(), "{tag}: chave vazia");
                assert!(!to.trim().is_empty(), "{tag}: tradução vazia de {from:?}");
            }
        }
    }

    #[test]
    fn every_catalog_translates_the_same_menu_titles() {
        let reference: Vec<String> = {
            let map: HashMap<String, String> = serde_json::from_str(EMBEDDED[0].1).unwrap();
            let mut keys: Vec<String> = map.into_keys().collect();
            keys.sort();
            keys
        };
        for (tag, text) in EMBEDDED {
            let map: HashMap<String, String> = serde_json::from_str(text).unwrap();
            let mut keys: Vec<String> = map.into_keys().collect();
            keys.sort();
            assert_eq!(
                keys, reference,
                "{tag} não cobre as mesmas entradas que en-US"
            );
        }
    }

    #[test]
    fn portuguese_and_unknown_languages_keep_the_text_and_a_known_one_translates_it() {
        // one test touches the global language, so it stays in one test
        set_language("pt-BR");
        assert_eq!(tr("Arquivo"), "Arquivo");
        set_language("en-US");
        assert_eq!(language(), "en-US");
        assert_eq!(tr("Arquivo"), "File");
        // a label with no entry is shown as written, not blank
        assert_eq!(
            tr("Texto que ninguém traduziu"),
            "Texto que ninguém traduziu"
        );
        // case is ignored and an unknown tag falls back to Portuguese
        set_language("EN-us");
        assert_eq!(language(), "en-US");
        set_language("klingon");
        assert_eq!(language(), "pt-BR");
        assert_eq!(tr("Arquivo"), "Arquivo");
        // Japanese has no catalog yet: the text stays Portuguese
        set_language("ja-JP");
        assert_eq!(language(), "ja-JP");
        assert_eq!(tr("Arquivo"), "Arquivo");
        set_language("pt-BR");
    }

    #[test]
    fn the_offered_languages_match_the_preferences_selector() {
        assert_eq!(LANGUAGES[0], "pt-BR");
        assert_eq!(LANGUAGES.len(), 8);
    }
}
