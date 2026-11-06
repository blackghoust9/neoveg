//! Media Profiles Rule Matcher (`profiles.ini` per results.md §18.4).
//!
//! Evaluates incoming media streams against consecutively numbered KeyN and AttributesN
//! rules in REVERSE order (from highest N down to 1). Wildcard values (0, 0.0, "None", "")
//! match any stream property. Frame rates are compared with an epsilon of 0.005.

use crate::media_props::{AlphaMode, FieldOrder};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Matching criteria from KeyN in profiles.ini.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaProfileKey {
    pub file_io_clsid: Option<String>,
    pub subformat: String,
    pub size_x: u32,
    pub size_y: u32,
    pub frame_rate: f64,
    pub color_bit_depth: u32,
}

impl MediaProfileKey {
    pub fn new(
        file_io_clsid: Option<String>,
        subformat: impl Into<String>,
        size_x: u32,
        size_y: u32,
        frame_rate: f64,
        color_bit_depth: u32,
    ) -> Self {
        Self {
            file_io_clsid,
            subformat: subformat.into(),
            size_x,
            size_y,
            frame_rate,
            color_bit_depth,
        }
    }

    pub fn wildcard() -> Self {
        Self {
            file_io_clsid: None,
            subformat: String::new(),
            size_x: 0,
            size_y: 0,
            frame_rate: 0.0,
            color_bit_depth: 0,
        }
    }
}

/// Stream attribute overrides from AttributesN in profiles.ini.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaProfileAttributes {
    pub interlace: FieldOrder,
    pub pixel_aspect_ratio: f64,
    pub alpha_interpretation: AlphaMode,
    pub gamma: f64,
}

impl MediaProfileAttributes {
    pub fn new(
        interlace: FieldOrder,
        pixel_aspect_ratio: f64,
        alpha_interpretation: AlphaMode,
        gamma: f64,
    ) -> Self {
        Self {
            interlace,
            pixel_aspect_ratio,
            alpha_interpretation,
            gamma,
        }
    }
}

/// One numbered rule pairing KeyN and AttributesN.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaProfileRule {
    pub index: usize,
    pub key: MediaProfileKey,
    pub attributes: MediaProfileAttributes,
}

/// Reverse-order rule matcher for media profiles.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaProfileMatcher {
    rules: Vec<MediaProfileRule>,
}

impl MediaProfileMatcher {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn rules(&self) -> &[MediaProfileRule] {
        &self.rules
    }

    pub fn add_rule(&mut self, key: MediaProfileKey, attributes: MediaProfileAttributes) {
        let index = self.rules.len() + 1;
        self.rules.push(MediaProfileRule {
            index,
            key,
            attributes,
        });
    }

    /// Parses an INI configuration string into a `MediaProfileMatcher`.
    ///
    /// The parser expects a `[Video Media Profiles]` section with `KeyN` and `AttributesN`
    /// entries where N = 1, 2, 3...
    /// Scanning strictly terminates at the first gap in rule numbers (Edge Case 11).
    pub fn parse_ini(content: &str) -> Result<Self, String> {
        let mut matcher = Self::new();
        let mut in_section = false;
        let mut keys = std::collections::HashMap::new();
        let mut attrs = std::collections::HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                let section_name = &line[1..line.len() - 1].trim();
                in_section = section_name.eq_ignore_ascii_case("Video Media Profiles");
                continue;
            }
            if !in_section {
                continue;
            }

            if let Some((k, v)) = line.split_once('=') {
                let key_name = k.trim();
                let val = v.trim();

                if let Some(idx_str) = key_name.strip_prefix("Key") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        keys.insert(idx, val.to_string());
                    }
                } else if let Some(idx_str) = key_name.strip_prefix("Attributes") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        attrs.insert(idx, val.to_string());
                    }
                }
            }
        }

        // Sequential index check: must start at 1 and stop at the first gap.
        let mut current_idx = 1;
        while let (Some(key_str), Some(attr_str)) = (keys.get(&current_idx), attrs.get(&current_idx)) {
            let key = parse_key(key_str)?;
            let attr = parse_attributes(attr_str)?;
            matcher.rules.push(MediaProfileRule {
                index: current_idx,
                key,
                attributes: attr,
            });
            current_idx += 1;
        }

        Ok(matcher)
    }

    /// Loads and parses profiles from a filesystem path.
    pub fn load_file(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read profiles.ini at {}: {e}", path.display()))?;
        Self::parse_ini(&content)
    }

    /// Evaluates stream properties against rules in REVERSE order (highest index down to 1).
    ///
    /// Later rules take priority over earlier ones (Edge Case 12).
    /// Wildcards `0` and `"None"` match any value.
    /// FPS comparison applies an epsilon tolerance of 0.005 (Edge Case 13).
    pub fn match_stream(&self, target: &MediaProfileKey) -> Option<MediaProfileAttributes> {
        for rule in self.rules.iter().rev() {
            let clsid_match = match (&rule.key.file_io_clsid, &target.file_io_clsid) {
                (None, _) => true,
                (Some(k_clsid), Some(t_clsid)) => {
                    k_clsid == "0" || k_clsid.eq_ignore_ascii_case("None") || k_clsid == t_clsid
                }
                (Some(k_clsid), None) => k_clsid == "0" || k_clsid.eq_ignore_ascii_case("None"),
            };

            let subformat_match = rule.key.subformat.is_empty()
                || rule.key.subformat == "0"
                || rule.key.subformat.eq_ignore_ascii_case("None")
                || target.subformat.is_empty()
                || rule.key.subformat.eq_ignore_ascii_case(&target.subformat);

            let size_match = (rule.key.size_x == 0
                || target.size_x == 0
                || rule.key.size_x == target.size_x)
                && (rule.key.size_y == 0
                    || target.size_y == 0
                    || rule.key.size_y == target.size_y);

            let fps_match = rule.key.frame_rate <= 0.0
                || target.frame_rate <= 0.0
                || (rule.key.frame_rate - target.frame_rate).abs() < 0.005;

            let depth_match = rule.key.color_bit_depth == 0
                || target.color_bit_depth == 0
                || rule.key.color_bit_depth == target.color_bit_depth;

            if clsid_match && subformat_match && size_match && fps_match && depth_match {
                return Some(rule.attributes.clone());
            }
        }
        None
    }

    /// Matches raw stream parameters.
    pub fn match_raw(
        &self,
        clsid: Option<&str>,
        subformat: Option<&str>,
        width: u32,
        height: u32,
        fps: f64,
        bit_depth: u32,
    ) -> Option<MediaProfileAttributes> {
        let key = MediaProfileKey {
            file_io_clsid: clsid.map(ToString::to_string),
            subformat: subformat.unwrap_or("").to_string(),
            size_x: width,
            size_y: height,
            frame_rate: fps,
            color_bit_depth: bit_depth,
        };
        self.match_stream(&key)
    }

    /// Returns the default stock media profiles catalog matching results.md §18.4 lines 20111–20140.
    pub fn default_built_in() -> Self {
        let mut m = Self::new();
        // 1. DV NTSC
        m.add_rule(
            MediaProfileKey::new(None, "None", 720, 480, 29.970030, 0),
            MediaProfileAttributes::new(FieldOrder::LowerFirst, 0.909091, AlphaMode::None, 2.222222),
        );
        // 2. DV PAL
        m.add_rule(
            MediaProfileKey::new(None, "None", 720, 576, 25.000000, 0),
            MediaProfileAttributes::new(FieldOrder::LowerFirst, 1.092593, AlphaMode::None, 2.222222),
        );
        // 3. DV NTSC Widescreen
        m.add_rule(
            MediaProfileKey::new(None, "DV Widescreen", 720, 480, 29.970030, 0),
            MediaProfileAttributes::new(FieldOrder::LowerFirst, 1.212121, AlphaMode::None, 2.222222),
        );
        // 4. DV PAL Widescreen
        m.add_rule(
            MediaProfileKey::new(None, "DV Widescreen", 720, 576, 25.000000, 0),
            MediaProfileAttributes::new(FieldOrder::LowerFirst, 1.456790, AlphaMode::None, 2.222222),
        );
        // 5. HDV 1080i NTSC
        m.add_rule(
            MediaProfileKey::new(None, "None", 1440, 1080, 29.970030, 0),
            MediaProfileAttributes::new(FieldOrder::UpperFirst, 1.333333, AlphaMode::None, 2.222222),
        );
        // 6. HDV 1080i PAL
        m.add_rule(
            MediaProfileKey::new(None, "None", 1440, 1080, 25.000000, 0),
            MediaProfileAttributes::new(FieldOrder::UpperFirst, 1.333333, AlphaMode::None, 2.222222),
        );
        // 7. HDV 720p 24
        m.add_rule(
            MediaProfileKey::new(None, "None", 1280, 720, 23.976024, 0),
            MediaProfileAttributes::new(FieldOrder::Progressive, 1.000000, AlphaMode::None, 2.222222),
        );
        // 8. HDV 720p 25
        m.add_rule(
            MediaProfileKey::new(None, "None", 1280, 720, 25.000000, 0),
            MediaProfileAttributes::new(FieldOrder::Progressive, 1.000000, AlphaMode::None, 2.222222),
        );
        // 9. HDV 720p 30
        m.add_rule(
            MediaProfileKey::new(None, "None", 1280, 720, 29.970030, 0),
            MediaProfileAttributes::new(FieldOrder::Progressive, 1.000000, AlphaMode::None, 2.222222),
        );
        // 10. HDV 720p 50
        m.add_rule(
            MediaProfileKey::new(None, "None", 1280, 720, 50.000000, 0),
            MediaProfileAttributes::new(FieldOrder::Progressive, 1.000000, AlphaMode::None, 2.222222),
        );
        // 11. HDV 720p 60
        m.add_rule(
            MediaProfileKey::new(None, "None", 1280, 720, 59.940060, 0),
            MediaProfileAttributes::new(FieldOrder::Progressive, 1.000000, AlphaMode::None, 2.222222),
        );
        // 12. 1080p Standard Square
        m.add_rule(
            MediaProfileKey::new(None, "None", 1920, 1080, 0.0, 0),
            MediaProfileAttributes::new(FieldOrder::Progressive, 1.000000, AlphaMode::Auto, 2.222222),
        );
        // 13. 4K UHD Standard Square
        m.add_rule(
            MediaProfileKey::new(None, "None", 3840, 2160, 0.0, 0),
            MediaProfileAttributes::new(FieldOrder::Progressive, 1.000000, AlphaMode::Auto, 2.222222),
        );
        m
    }
}

fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn parse_key(val: &str) -> Result<MediaProfileKey, String> {
    let parts: Vec<&str> = val.split(',').map(str::trim).collect();
    if parts.len() < 6 {
        return Err(format!("Key requires 6 comma-separated fields, got: {val}"));
    }
    let clsid_raw = strip_quotes(parts[0]);
    let file_io_clsid = if clsid_raw == "0" || clsid_raw.eq_ignore_ascii_case("None") || clsid_raw.is_empty() {
        None
    } else {
        Some(clsid_raw.to_string())
    };

    let subformat_raw = strip_quotes(parts[1]);
    let subformat = if subformat_raw == "0" || subformat_raw.eq_ignore_ascii_case("None") {
        String::new()
    } else {
        subformat_raw.to_string()
    };

    let size_x: u32 = parts[2].parse().unwrap_or(0);
    let size_y: u32 = parts[3].parse().unwrap_or(0);
    let frame_rate: f64 = parts[4].parse().unwrap_or(0.0);
    let color_bit_depth: u32 = parts[5].parse().unwrap_or(0);

    Ok(MediaProfileKey {
        file_io_clsid,
        subformat,
        size_x,
        size_y,
        frame_rate,
        color_bit_depth,
    })
}

fn parse_attributes(val: &str) -> Result<MediaProfileAttributes, String> {
    let parts: Vec<&str> = val.split(',').map(str::trim).collect();
    if parts.len() < 4 {
        return Err(format!("Attributes requires 4 comma-separated fields, got: {val}"));
    }

    let interlace_str = strip_quotes(parts[0]);
    let interlace = match interlace_str {
        s if s.eq_ignore_ascii_case("Lower field first") || s.eq_ignore_ascii_case("LowerFirst") => {
            FieldOrder::LowerFirst
        }
        s if s.eq_ignore_ascii_case("Upper field first") || s.eq_ignore_ascii_case("UpperFirst") => {
            FieldOrder::UpperFirst
        }
        _ => FieldOrder::Progressive,
    };

    let pixel_aspect_ratio: f64 = parts[1].parse().unwrap_or(1.0);

    let alpha_str = strip_quotes(parts[2]);
    let alpha_interpretation = match alpha_str {
        s if s.eq_ignore_ascii_case("Straight") => AlphaMode::Straight,
        s if s.eq_ignore_ascii_case("Premultiplied") => AlphaMode::Premultiplied,
        s if s.eq_ignore_ascii_case("Premultiplied (Dirty)")
            || s.eq_ignore_ascii_case("PremultipliedDirty") =>
        {
            AlphaMode::PremultipliedDirty
        }
        s if s.eq_ignore_ascii_case("Undefined") => AlphaMode::Undefined,
        s if s.eq_ignore_ascii_case("Auto") => AlphaMode::Auto,
        _ => AlphaMode::None,
    };

    let gamma: f64 = parts[3].parse().unwrap_or(2.222222);

    Ok(MediaProfileAttributes {
        interlace,
        pixel_aspect_ratio,
        alpha_interpretation,
        gamma,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ini_and_matches_in_reverse_order() {
        let ini = r#"
[Video Media Profiles]
Key1 = 0, "None", 720, 480, 29.970030, 0
Attributes1 = "Lower field first", 0.909091, "None", 2.222222
Key2 = 0, "None", 720, 480, 29.970030, 0
Attributes2 = "Progressive", 1.212121, "None", 2.222222
"#;
        let matcher = MediaProfileMatcher::parse_ini(ini).expect("parse");
        assert_eq!(matcher.rules().len(), 2);

        // Later rule Key2 must override Key1 (Edge Case 12)
        let query = MediaProfileKey::new(None, "None", 720, 480, 29.97, 8);
        let matched = matcher.match_stream(&query).expect("match");
        assert_eq!(matched.interlace, FieldOrder::Progressive);
        assert!((matched.pixel_aspect_ratio - 1.212121).abs() < 1e-4);
    }

    #[test]
    fn stops_at_gap_in_rule_numbering() {
        let ini = r#"
[Video Media Profiles]
Key1 = 0, "None", 720, 480, 29.97, 0
Attributes1 = "Lower field first", 0.909091, "None", 2.222
Key2 = 0, "None", 1920, 1080, 24.0, 0
Attributes2 = "Progressive", 1.0, "None", 2.222
Key4 = 0, "None", 3840, 2160, 60.0, 0
Attributes4 = "Progressive", 1.0, "None", 2.222
"#;
        let matcher = MediaProfileMatcher::parse_ini(ini).expect("parse");
        // Key3 missing, must stop at Key2 (Edge Case 11)
        assert_eq!(matcher.rules().len(), 2);
    }

    #[test]
    fn fps_epsilon_tolerance_matches() {
        let mut matcher = MediaProfileMatcher::new();
        matcher.add_rule(
            MediaProfileKey::new(None, "", 1920, 1080, 29.970, 0),
            MediaProfileAttributes::new(FieldOrder::Progressive, 1.0, AlphaMode::None, 2.222),
        );

        // Query with 29.970030 (difference < 0.005)
        let query = MediaProfileKey::new(None, "", 1920, 1080, 29.970030, 8);
        assert!(matcher.match_stream(&query).is_some());

        // Query with 29.980 (difference > 0.005)
        let query_mismatch = MediaProfileKey::new(None, "", 1920, 1080, 29.980, 8);
        assert!(matcher.match_stream(&query_mismatch).is_none());
    }

    #[test]
    fn dirty_alpha_parsed_correctly() {
        let ini = r#"
[Video Media Profiles]
Key1 = 0, "None", 1920, 1080, 0, 0
Attributes1 = "Progressive", 1.0, "Premultiplied (Dirty)", 2.222
"#;
        let matcher = MediaProfileMatcher::parse_ini(ini).expect("parse");
        let matched = matcher
            .match_raw(None, None, 1920, 1080, 24.0, 8)
            .expect("matched");
        assert_eq!(matched.alpha_interpretation, AlphaMode::PremultipliedDirty);
    }
}
