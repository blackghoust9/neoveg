//! Styles of titles (manual v21, "Titles & Text"; the idea of saving the look of a title comes from
//! the text styles of the Premiere Pro reference): a name for the font, size, colors, outline,
//! shadow, position and animation of a title, without its text. A style is applied to the title
//! being made or edited in the media generator and is kept for every project.
//!
//! The generator itself is in `neoveg_video`; here the style holds plain values (the animation by
//! its snake-case id) and the dialog turns them into the types of the generator.

use serde::{Deserialize, Serialize};

/// Most styles kept.
pub const MAX_STYLES: usize = 128;
/// Longest name, in characters.
pub const MAX_NAME: usize = 60;
/// Longest font family or style name, in characters.
pub const MAX_FONT: usize = 80;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleStyle {
    pub name: String,
    pub font: String,
    pub font_style: String,
    /// Size of the font, in the units of the generator (2 to 16 in the dialog).
    pub scale: u32,
    pub text_rgba: [u8; 4],
    pub bg_rgba: [u8; 4],
    pub outline_rgba: [u8; 4],
    pub outline_px: u32,
    pub shadow_rgba: [u8; 4],
    pub shadow_offset: [i32; 2],
    /// Vertical position, 0.5 is the centre.
    pub align_y: f32,
    /// `static`, `fade_in`, `typewriter`, `slide_up` or `credit_roll`.
    pub animation: String,
}

impl Default for TitleStyle {
    fn default() -> Self {
        Self {
            name: String::new(),
            font: "Inter".into(),
            font_style: "Bold".into(),
            scale: 6,
            text_rgba: [255, 255, 255, 255],
            bg_rgba: [0, 0, 0, 0],
            outline_rgba: [14, 16, 22, 255],
            outline_px: 2,
            shadow_rgba: [0, 0, 0, 180],
            shadow_offset: [4, 4],
            align_y: 0.5,
            animation: "static".into(),
        }
    }
}

/// The styles that come with the program, in the order they are listed. They cannot be deleted; a
/// style the user saves under the same name hides the built-in one.
pub fn builtin() -> Vec<TitleStyle> {
    vec![
        TitleStyle {
            name: "Título central".into(),
            ..TitleStyle::default()
        },
        TitleStyle {
            name: "Terço inferior".into(),
            scale: 4,
            align_y: 0.82,
            bg_rgba: [0, 0, 0, 140],
            animation: "slide_up".into(),
            ..TitleStyle::default()
        },
        TitleStyle {
            name: "Legenda".into(),
            scale: 3,
            align_y: 0.9,
            font_style: "Regular".into(),
            outline_px: 3,
            shadow_offset: [2, 2],
            ..TitleStyle::default()
        },
        TitleStyle {
            name: "Créditos".into(),
            font: "Noto Serif".into(),
            font_style: "Regular".into(),
            scale: 4,
            outline_px: 0,
            shadow_rgba: [0, 0, 0, 0],
            animation: "credit_roll".into(),
            ..TitleStyle::default()
        },
    ]
}

/// The name as it is kept: trimmed and at most [`MAX_NAME`] characters; `None` when nothing is left.
pub fn clean_name(name: &str) -> Option<String> {
    let name: String = name.trim().chars().take(MAX_NAME).collect();
    (!name.is_empty()).then_some(name)
}

/// The style with every value inside what the generator takes; `None` without a name.
pub fn sanitized(style: &TitleStyle) -> Option<TitleStyle> {
    let name = clean_name(&style.name)?;
    let defaults = TitleStyle::default();
    let text = |value: &str, fallback: &str| -> String {
        let value: String = value.trim().chars().take(MAX_FONT).collect();
        if value.is_empty() {
            fallback.to_owned()
        } else {
            value
        }
    };
    let animation = match style.animation.as_str() {
        known @ ("static" | "fade_in" | "typewriter" | "slide_up" | "credit_roll") => known.into(),
        _ => defaults.animation.clone(),
    };
    Some(TitleStyle {
        name,
        font: text(&style.font, &defaults.font),
        font_style: text(&style.font_style, &defaults.font_style),
        scale: style.scale.clamp(1, 20),
        text_rgba: style.text_rgba,
        bg_rgba: style.bg_rgba,
        outline_rgba: style.outline_rgba,
        outline_px: style.outline_px.min(12),
        shadow_rgba: style.shadow_rgba,
        shadow_offset: style.shadow_offset.map(|v| v.clamp(-24, 24)),
        align_y: if style.align_y.is_finite() {
            style.align_y.clamp(0.05, 0.95)
        } else {
            defaults.align_y
        },
        animation,
    })
}

/// Saves `style` in `list`: one with the same name (ignoring case) is replaced, a new one goes to
/// the end. `false` when it has no name or the list is full.
pub fn save(list: &mut Vec<TitleStyle>, style: &TitleStyle) -> bool {
    let Some(kept) = sanitized(style) else {
        return false;
    };
    let same = list
        .iter()
        .position(|s| s.name.to_lowercase() == kept.name.to_lowercase());
    match same {
        Some(index) => list[index] = kept,
        None if list.len() >= MAX_STYLES => return false,
        None => list.push(kept),
    }
    true
}

/// Deletes the style called `name`; `true` when there was one.
pub fn remove(list: &mut Vec<TitleStyle>, name: &str) -> bool {
    let before = list.len();
    list.retain(|s| s.name != name);
    list.len() != before
}

/// The built-in styles that the user has not replaced, then the saved ones.
pub fn all(saved: &[TitleStyle]) -> Vec<TitleStyle> {
    let mut styles: Vec<TitleStyle> = builtin()
        .into_iter()
        .filter(|b| {
            !saved
                .iter()
                .any(|s| s.name.to_lowercase() == b.name.to_lowercase())
        })
        .collect();
    styles.extend(saved.iter().cloned());
    styles
}

/// Whether `name` is the name of a built-in style that has not been replaced.
pub fn is_builtin(saved: &[TitleStyle], name: &str) -> bool {
    builtin().iter().any(|b| b.name == name)
        && !saved
            .iter()
            .any(|s| s.name.to_lowercase() == name.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_is_saved_replaced_by_name_and_removed() {
        let mut list = Vec::new();
        let style = TitleStyle {
            name: "Meu".into(),
            scale: 9,
            ..TitleStyle::default()
        };
        assert!(save(&mut list, &style));
        assert!(save(
            &mut list,
            &TitleStyle {
                name: " meu ".into(),
                scale: 3,
                ..TitleStyle::default()
            }
        ));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].scale, 3);
        assert!(!save(&mut list, &TitleStyle::default()), "no name");
        assert!(remove(&mut list, "meu"));
        assert!(!remove(&mut list, "meu"));
    }

    #[test]
    fn values_are_made_to_fit_what_the_generator_takes() {
        let odd = TitleStyle {
            name: "x".into(),
            font: "  ".into(),
            scale: 500,
            outline_px: 99,
            shadow_offset: [-90, 90],
            align_y: f32::NAN,
            animation: "explode".into(),
            ..TitleStyle::default()
        };
        let kept = sanitized(&odd).unwrap();
        assert_eq!(kept.font, "Inter");
        assert_eq!((kept.scale, kept.outline_px), (20, 12));
        assert_eq!(kept.shadow_offset, [-24, 24]);
        assert_eq!(kept.align_y, 0.5);
        assert_eq!(kept.animation, "static");
    }

    #[test]
    fn the_built_in_styles_come_first_and_a_saved_one_with_the_same_name_replaces_them() {
        let names: Vec<String> = all(&[]).into_iter().map(|s| s.name).collect();
        assert_eq!(names.len(), builtin().len());
        let mine = TitleStyle {
            name: "legenda".into(),
            scale: 7,
            ..TitleStyle::default()
        };
        let merged = all(std::slice::from_ref(&mine));
        assert_eq!(merged.len(), builtin().len(), "one replaced, none added");
        assert_eq!(merged.last().unwrap().scale, 7);
        assert!(is_builtin(&[], "Legenda"));
        assert!(!is_builtin(std::slice::from_ref(&mine), "Legenda"));
        assert!(!is_builtin(&[], "Meu"));
        for style in builtin() {
            assert_eq!(sanitized(&style).as_ref(), Some(&style), "{}", style.name);
        }
    }
}
