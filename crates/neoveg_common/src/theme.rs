//! Interface themes: the four built-in palettes and user themes loaded from `.json` files.
//!
//! The palettes are Neoveg's own colors, named after the classic NLE themes they echo. A user theme
//! is a JSON file with a `format_version`; every color is optional (a missing one keeps the value of
//! the Modern Carbon Dark base) so files written for an older version keep loading.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::Path;

/// The newest theme file version this build reads and writes.
pub const THEME_FORMAT_VERSION: u32 = 1;

/// An opaque color written as `#RRGGBB` in theme files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub [u8; 3]);

impl Rgb {
    /// Parses `#RRGGBB` or `RRGGBB` (also the short `#RGB`).
    pub fn parse(text: &str) -> Result<Self, String> {
        let hex = text.trim().trim_start_matches('#');
        let digit = |c: char| {
            c.to_digit(16)
                .map(|d| d as u8)
                .ok_or_else(|| format!("cor inválida: \"{text}\""))
        };
        let chars: Vec<char> = hex.chars().collect();
        match chars.len() {
            6 => {
                let mut out = [0u8; 3];
                for (i, pair) in chars.chunks(2).enumerate() {
                    out[i] = digit(pair[0])? * 16 + digit(pair[1])?;
                }
                Ok(Self(out))
            }
            3 => {
                let mut out = [0u8; 3];
                for (i, c) in chars.iter().enumerate() {
                    out[i] = digit(*c)? * 17;
                }
                Ok(Self(out))
            }
            _ => Err(format!("cor inválida: \"{text}\" (use #RRGGBB)")),
        }
    }

    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0[0], self.0[1], self.0[2])
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::parse(&text).map_err(D::Error::custom)
    }
}

/// Every color the interface takes from a theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeColors {
    /// Window and docked-panel background.
    pub panel: Rgb,
    /// Text fields, scroll areas and other recessed surfaces.
    pub recessed: Rgb,
    /// Buttons and widgets at rest.
    pub widget: Rgb,
    pub widget_hover: Rgb,
    pub widget_active: Rgb,
    /// Body text.
    pub text: Rgb,
    /// Selection, links and the playhead.
    pub accent: Rgb,
    /// Timeline background, lanes (two alternating shades), ruler and grid.
    pub timeline_bg: Rgb,
    pub lane: Rgb,
    pub lane_alt: Rgb,
    pub grid_minor: Rgb,
    pub grid_major: Rgb,
    /// Track header column and the selected row.
    pub header: Rgb,
    pub header_selected: Rgb,
    /// Event bodies on audio and video tracks.
    pub audio_clip: Rgb,
    pub video_clip: Rgb,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Theme::modern_carbon_dark().colors
    }
}

/// A complete interface theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub format_version: u32,
    pub name: String,
    /// Starting point for the widget styling: dark or light egui visuals.
    pub dark: bool,
    pub colors: ThemeColors,
}

impl Default for Theme {
    fn default() -> Self {
        Self::modern_carbon_dark()
    }
}

const fn c(r: u8, g: u8, b: u8) -> Rgb {
    Rgb([r, g, b])
}

/// Identifier stored in the preferences when the theme comes from a user file.
pub const CUSTOM_THEME_ID: &str = "custom";

/// Built-in theme ids, in menu order.
pub const BUILTIN_IDS: [&str; 4] = [
    "classic-foundry",
    "classic-silver",
    "modern-carbon-dark",
    "contemporary-electric-dark",
];

impl Theme {
    pub fn classic_foundry() -> Self {
        Self {
            format_version: THEME_FORMAT_VERSION,
            name: "Classic Foundry".into(),
            dark: true,
            colors: ThemeColors {
                panel: c(0x3A, 0x3B, 0x3D),
                recessed: c(0x26, 0x27, 0x29),
                widget: c(0x4A, 0x4B, 0x4E),
                widget_hover: c(0x5A, 0x5C, 0x60),
                widget_active: c(0x6B, 0x6D, 0x72),
                text: c(0xE2, 0xE2, 0xE2),
                accent: c(0xD9, 0x8A, 0x2B),
                timeline_bg: c(0x2B, 0x2C, 0x2E),
                lane: c(0x34, 0x35, 0x37),
                lane_alt: c(0x30, 0x31, 0x33),
                grid_minor: c(0x3C, 0x3D, 0x40),
                grid_major: c(0x58, 0x59, 0x5E),
                header: c(0x40, 0x41, 0x44),
                header_selected: c(0x58, 0x4F, 0x44),
                audio_clip: c(0x3F, 0x6E, 0x57),
                video_clip: c(0x6B, 0x57, 0x8E),
            },
        }
    }

    pub fn classic_silver() -> Self {
        Self {
            format_version: THEME_FORMAT_VERSION,
            name: "Classic Silver".into(),
            dark: false,
            colors: ThemeColors {
                panel: c(0xD6, 0xD8, 0xDB),
                recessed: c(0xF2, 0xF3, 0xF4),
                widget: c(0xC4, 0xC7, 0xCB),
                widget_hover: c(0xB4, 0xB8, 0xBD),
                widget_active: c(0xA2, 0xA7, 0xAD),
                text: c(0x1E, 0x20, 0x24),
                accent: c(0x1F, 0x6F, 0xB5),
                timeline_bg: c(0xB9, 0xBC, 0xC1),
                lane: c(0xCF, 0xD2, 0xD6),
                lane_alt: c(0xC6, 0xC9, 0xCE),
                grid_minor: c(0xBD, 0xC0, 0xC5),
                grid_major: c(0x8E, 0x93, 0x9A),
                header: c(0xC8, 0xCB, 0xCF),
                header_selected: c(0xA9, 0xC4, 0xDE),
                audio_clip: c(0x5B, 0x9B, 0xC7),
                video_clip: c(0x9A, 0x84, 0xC4),
            },
        }
    }

    pub fn modern_carbon_dark() -> Self {
        Self {
            format_version: THEME_FORMAT_VERSION,
            name: "Modern Carbon Dark".into(),
            dark: true,
            colors: ThemeColors {
                panel: c(0x1B, 0x1B, 0x1B),
                recessed: c(0x10, 0x10, 0x10),
                widget: c(0x2D, 0x2D, 0x2D),
                widget_hover: c(0x3C, 0x3C, 0x3C),
                widget_active: c(0x4B, 0x4B, 0x4B),
                text: c(0xD8, 0xD8, 0xD8),
                accent: c(0x29, 0x80, 0xB9),
                timeline_bg: c(26, 28, 33),
                lane: c(31, 33, 37),
                lane_alt: c(28, 30, 34),
                grid_minor: c(0x2C, 0x30, 0x38),
                grid_major: c(0x4A, 0x50, 0x5E),
                header: c(35, 38, 45),
                header_selected: c(48, 54, 66),
                audio_clip: c(38, 82, 120),
                video_clip: c(88, 66, 130),
            },
        }
    }

    pub fn contemporary_electric_dark() -> Self {
        Self {
            format_version: THEME_FORMAT_VERSION,
            name: "Contemporary Electric Dark".into(),
            dark: true,
            colors: ThemeColors {
                panel: c(0x14, 0x18, 0x24),
                recessed: c(0x0B, 0x0E, 0x18),
                widget: c(0x22, 0x29, 0x3D),
                widget_hover: c(0x2E, 0x38, 0x52),
                widget_active: c(0x3A, 0x47, 0x69),
                text: c(0xE4, 0xEA, 0xF6),
                accent: c(0x00, 0xD1, 0xFF),
                timeline_bg: c(0x0F, 0x13, 0x1E),
                lane: c(0x17, 0x1C, 0x2A),
                lane_alt: c(0x13, 0x18, 0x25),
                grid_minor: c(0x1F, 0x26, 0x38),
                grid_major: c(0x36, 0x42, 0x5F),
                header: c(0x1A, 0x20, 0x31),
                header_selected: c(0x1E, 0x3A, 0x55),
                audio_clip: c(0x0E, 0x7C, 0xA8),
                video_clip: c(0x7A, 0x4C, 0xD6),
            },
        }
    }

    /// The built-in theme with this id.
    pub fn builtin(id: &str) -> Option<Self> {
        match id {
            "classic-foundry" => Some(Self::classic_foundry()),
            "classic-silver" => Some(Self::classic_silver()),
            "modern-carbon-dark" => Some(Self::modern_carbon_dark()),
            "contemporary-electric-dark" => Some(Self::contemporary_electric_dark()),
            _ => None,
        }
    }

    /// Parses a theme file. Never panics; files from a newer version are refused.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let theme: Self = serde_json::from_str(text).map_err(|e| format!("tema inválido: {e}"))?;
        if theme.format_version > THEME_FORMAT_VERSION {
            return Err(format!(
                "tema escrito por uma versão mais nova do Neoveg (formato {})",
                theme.format_version
            ));
        }
        Ok(theme)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?;
        Self::from_json(&text)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_json())
    }
}

/// Display name of a built-in id.
pub fn builtin_name(id: &str) -> &'static str {
    match id {
        "classic-foundry" => "Classic Foundry",
        "classic-silver" => "Classic Silver",
        "modern-carbon-dark" => "Modern Carbon Dark",
        "contemporary-electric-dark" => "Contemporary Electric Dark",
        _ => "Personalizado",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_parse_long_short_and_reject_garbage() {
        assert_eq!(Rgb::parse("#2980B9"), Ok(Rgb([0x29, 0x80, 0xB9])));
        assert_eq!(Rgb::parse("2980b9"), Ok(Rgb([0x29, 0x80, 0xB9])));
        assert_eq!(Rgb::parse("#fa0"), Ok(Rgb([0xFF, 0xAA, 0x00])));
        assert!(Rgb::parse("#12345").is_err());
        assert!(Rgb::parse("#GGGGGG").is_err());
        assert!(Rgb::parse("").is_err());
        assert!(Rgb::parse("#ééé").is_err());
    }

    #[test]
    fn every_builtin_id_resolves_and_round_trips() {
        for id in BUILTIN_IDS {
            let theme = Theme::builtin(id).expect(id);
            assert_eq!(theme.name, builtin_name(id));
            assert_eq!(Theme::from_json(&theme.to_json()), Ok(theme));
        }
        assert!(Theme::builtin("nope").is_none());
    }

    #[test]
    fn builtin_themes_differ_and_silver_is_light() {
        let all: Vec<Theme> = BUILTIN_IDS
            .iter()
            .filter_map(|i| Theme::builtin(i))
            .collect();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.colors, b.colors);
            }
        }
        assert!(!Theme::classic_silver().dark);
        assert!(Theme::modern_carbon_dark().dark);
    }

    #[test]
    fn a_partial_older_file_keeps_loading_with_defaults() {
        let theme = Theme::from_json(
            r##"{"format_version":1,"name":"Meu","colors":{"accent":"#FF0000"}}"##,
        )
        .unwrap();
        assert_eq!(theme.name, "Meu");
        assert_eq!(theme.colors.accent, Rgb([255, 0, 0]));
        assert_eq!(theme.colors.panel, Theme::modern_carbon_dark().colors.panel);
        assert!(Theme::from_json("{}").is_ok());
    }

    #[test]
    fn bad_files_are_errors_not_panics() {
        assert!(Theme::from_json("not json").is_err());
        assert!(Theme::from_json(r##"{"colors":{"accent":"red"}}"##).is_err());
        assert!(Theme::from_json(r##"{"format_version":99}"##)
            .unwrap_err()
            .contains("mais nova"));
        assert!(Theme::load(Path::new("/definitely/missing.json")).is_err());
    }

    #[test]
    fn save_and_load_round_trip_through_a_file() {
        let dir = std::env::temp_dir().join(format!("neoveg_theme_{}", std::process::id()));
        let path = dir.join("tema.json");
        let theme = Theme::contemporary_electric_dark();
        theme.save(&path).unwrap();
        assert_eq!(Theme::load(&path), Ok(theme));
        let _ = std::fs::remove_dir_all(dir);
    }
}
