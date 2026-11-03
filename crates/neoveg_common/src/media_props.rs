//! What the user says about a media file that the file does not say itself (the Media Properties
//! dialog): a start timecode of its own, a substitute frame rate, the field order, the pixel aspect
//! ratio, how to read its alpha channel and a background colour for the transparent parts.
//!
//! The values are kept per media in the project and are plain data; the picture-related ones are
//! applied to each decoded frame by `neoveg_video::media_look`.

use serde::{Deserialize, Serialize};

/// Which field of an interlaced picture comes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldOrder {
    /// No interlacing (progressive scan).
    #[default]
    Progressive,
    UpperFirst,
    LowerFirst,
}

impl FieldOrder {
    pub const ALL: [FieldOrder; 3] = [Self::Progressive, Self::UpperFirst, Self::LowerFirst];
}

/// How the alpha channel of the file is to be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaMode {
    /// What the decoder found in the file.
    #[default]
    Auto,
    /// Ignore any alpha channel the file has.
    Undefined,
    /// The picture is opaque.
    None,
    /// Transparency is only in the alpha channel (straight, unmatted).
    Straight,
    /// The colour channels are already multiplied by the alpha.
    Premultiplied,
    /// Premultiplied, but a colour channel may be greater than the alpha.
    PremultipliedDirty,
}

impl AlphaMode {
    pub const ALL: [AlphaMode; 6] = [
        Self::Auto,
        Self::Undefined,
        Self::None,
        Self::Straight,
        Self::Premultiplied,
        Self::PremultipliedDirty,
    ];
}

/// The frame rates a media file can be conformed to.
pub const SUBSTITUTE_FPS: [f64; 8] = [23.976, 24.0, 25.0, 29.97, 30.0, 50.0, 59.94, 60.0];

/// Pixel aspect ratios the dialog offers: the value and its name (the square one is named by the
/// dialog, in the user's language).
pub const PIXEL_ASPECTS: [(f64, &str); 6] = [
    (0.9091, "NTSC DV"),
    (1.2121, "NTSC DV 16:9"),
    (1.0940, "PAL DV"),
    (1.4587, "PAL DV 16:9"),
    (1.3333, "HDV 1080"),
    (2.0, "2:1"),
];

/// Smallest and largest pixel aspect ratio accepted.
pub const PIXEL_ASPECT_RANGE: (f64, f64) = (0.25, 4.0);

fn square() -> f64 {
    1.0
}

fn is_square(value: &f64) -> bool {
    (*value - 1.0).abs() < 1e-9
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MediaProps {
    /// A start timecode of the user's (in seconds); `None` uses the one in the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timecode_start: Option<f64>,
    /// Frame rate the picture is played at instead of the file's own; `None` keeps the file's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substitute_fps: Option<f64>,
    #[serde(skip_serializing_if = "is_progressive")]
    pub field_order: FieldOrder,
    /// Width of a pixel over its height (`1.0`: square pixels).
    #[serde(default = "square", skip_serializing_if = "is_square")]
    pub pixel_aspect: f64,
    #[serde(skip_serializing_if = "is_auto")]
    pub alpha: AlphaMode,
    /// Colour the transparent parts of the picture are filled with; `None` leaves them transparent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<[u8; 3]>,
}

fn is_progressive(order: &FieldOrder) -> bool {
    *order == FieldOrder::Progressive
}

fn is_auto(mode: &AlphaMode) -> bool {
    *mode == AlphaMode::Auto
}

impl Default for MediaProps {
    fn default() -> Self {
        Self {
            timecode_start: None,
            substitute_fps: None,
            field_order: FieldOrder::Progressive,
            pixel_aspect: 1.0,
            alpha: AlphaMode::Auto,
            background: None,
        }
    }
}

impl MediaProps {
    /// `true` when nothing differs from what the file says (the project then keeps no entry).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The same values made safe: finite numbers, a pixel aspect within [`PIXEL_ASPECT_RANGE`], a
    /// substitute rate above zero, a start timecode that is not negative.
    pub fn sanitized(&self) -> Self {
        let mut out = self.clone();
        out.timecode_start = out
            .timecode_start
            .filter(|t| t.is_finite())
            .map(|t| t.max(0.0));
        out.substitute_fps = out.substitute_fps.filter(|f| f.is_finite() && *f > 0.0);
        out.pixel_aspect = if out.pixel_aspect.is_finite() {
            out.pixel_aspect
                .clamp(PIXEL_ASPECT_RANGE.0, PIXEL_ASPECT_RANGE.1)
        } else {
            1.0
        };
        out
    }

    /// How much faster the source plays than the file says: the substitute rate over the file's own
    /// (`1.0` without a substitute, or when the file's rate is not known).
    pub fn rate_factor(&self, file_fps: f64) -> f64 {
        match self.substitute_fps {
            Some(substitute) if substitute > 0.0 && file_fps > 0.0 => {
                (substitute / file_fps).clamp(0.1, 10.0)
            }
            _ => 1.0,
        }
    }

    /// `true` when the picture of each frame has to be changed (alpha, background or pixel aspect).
    pub fn changes_picture(&self) -> bool {
        self.alpha != AlphaMode::Auto || self.background.is_some() || !is_square(&self.pixel_aspect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_changes_nothing_and_is_not_written() {
        let props = MediaProps::default();
        assert!(props.is_default() && !props.changes_picture());
        assert_eq!(serde_json::to_string(&props).unwrap(), "{}");
        assert_eq!(props.rate_factor(24.0), 1.0);
        let back: MediaProps = serde_json::from_str("{}").unwrap();
        assert_eq!(back, props);
    }

    #[test]
    fn every_value_survives_the_project_file() {
        let props = MediaProps {
            timecode_start: Some(3600.0),
            substitute_fps: Some(25.0),
            field_order: FieldOrder::UpperFirst,
            pixel_aspect: 1.2121,
            alpha: AlphaMode::PremultipliedDirty,
            background: Some([10, 20, 30]),
        };
        let json = serde_json::to_string(&props).unwrap();
        assert!(json.contains("upper_first") && json.contains("premultiplied_dirty"));
        assert_eq!(serde_json::from_str::<MediaProps>(&json).unwrap(), props);
        assert!(!props.is_default() && props.changes_picture());
    }

    #[test]
    fn the_substitute_rate_speeds_the_source_up_by_its_ratio_and_odd_values_are_made_safe() {
        let props = MediaProps {
            substitute_fps: Some(30.0),
            ..MediaProps::default()
        };
        assert!((props.rate_factor(24.0) - 1.25).abs() < 1e-12);
        assert_eq!(props.rate_factor(0.0), 1.0, "unknown file rate");
        let wild = MediaProps {
            timecode_start: Some(-5.0),
            substitute_fps: Some(f64::NAN),
            pixel_aspect: 100.0,
            ..MediaProps::default()
        }
        .sanitized();
        assert_eq!(wild.timecode_start, Some(0.0));
        assert_eq!(wild.substitute_fps, None);
        assert_eq!(wild.pixel_aspect, PIXEL_ASPECT_RANGE.1);
        let nan = MediaProps {
            pixel_aspect: f64::NAN,
            ..MediaProps::default()
        }
        .sanitized();
        assert_eq!(nan.pixel_aspect, 1.0);
        assert!(
            MediaProps {
                substitute_fps: Some(1000.0),
                ..MediaProps::default()
            }
            .rate_factor(24.0)
                <= 10.0
        );
    }
}
