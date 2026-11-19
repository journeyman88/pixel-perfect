//! Typed worksheet model, deserialized from schema-valid data.
//!
//! Field names follow the schema exactly; `_*` reference fields are renamed to
//! plain Rust names (e.g. `_palette` -> `palette`).

use std::collections::BTreeMap;

use serde::Deserialize;

/// A whole worksheet.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Worksheet {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    #[serde(default)]
    pub palettes: Vec<Palette>,
    #[serde(default)]
    pub sprites: Vec<Sprite>,
    #[serde(default)]
    pub flipbooks: Vec<Flipbook>,
    #[serde(default)]
    pub panels: Vec<Panel>,
    #[serde(default)]
    pub timelines: Vec<Timeline>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    pub exports: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Palette {
    pub id: String,
    #[serde(rename = "_palette")]
    pub palette: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub colors: Vec<Color>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Color {
    pub char: char,
    pub rgba: Rgba,
}

/// An 8-bit RGBA color. Alpha is authored as 0.0-1.0 and converted with
/// `round_half_up(alpha * 255)`; it defaults to 255 (opaque).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "Vec<serde_json::Number>")]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    /// Converts an authored alpha (0.0-1.0) to 8 bits, rounding halves up.
    ///
    /// The small epsilon makes the result match the decimal value the author
    /// wrote: `0.3 * 255` is `76.4999...` in binary floating point but `76.5`
    /// in decimal, which rounds up to 77.
    pub fn alpha_to_u8(alpha: f64) -> u8 {
        (alpha.clamp(0.0, 1.0) * 255.0 + 0.5 + 1e-9)
            .floor()
            .min(255.0) as u8
    }
}

impl TryFrom<Vec<serde_json::Number>> for Rgba {
    type Error = String;

    fn try_from(channels: Vec<serde_json::Number>) -> Result<Self, Self::Error> {
        let channel = |i: usize| -> Result<u8, String> {
            channels[i]
                .as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .ok_or_else(|| format!("channel {i} must be an integer 0-255"))
        };
        match channels.len() {
            3 | 4 => {
                let alpha = match channels.get(3) {
                    Some(a) => Rgba::alpha_to_u8(a.as_f64().ok_or("alpha must be a number")?),
                    None => 255,
                };
                Ok(Rgba([channel(0)?, channel(1)?, channel(2)?, alpha]))
            }
            n => Err(format!("rgba must have 3 or 4 channels, found {n}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sprite {
    pub id: String,
    #[serde(rename = "_sprite")]
    pub sprite: Option<String>,
    #[serde(rename = "_palette")]
    pub palette: Option<String>,
    pub description: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub lines: Option<Vec<String>>,
    /// Keyed by the 0-based row index as written in the worksheet.
    pub override_lines: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u16")]
pub enum Rotation {
    Deg90,
    Deg180,
    Deg270,
}

impl TryFrom<u16> for Rotation {
    type Error = String;

    fn try_from(degrees: u16) -> Result<Self, Self::Error> {
        match degrees {
            90 => Ok(Rotation::Deg90),
            180 => Ok(Rotation::Deg180),
            270 => Ok(Rotation::Deg270),
            d => Err(format!("rotation must be 90, 180 or 270, found {d}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Transform {
    Flip {
        axis: Axis,
    },
    Rotate {
        degrees: Rotation,
    },
    Scale {
        width: u32,
        height: u32,
    },
    /// `[x, y, width, height]`.
    Crop {
        area: [u32; 4],
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Flipbook {
    pub id: String,
    pub description: Option<String>,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Frame {
    #[serde(rename = "_sprite")]
    pub sprite: String,
    pub coordinates: [i64; 2],
    #[serde(default)]
    pub transforms: Vec<Transform>,
    pub duration: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Element {
    pub id: Option<String>,
    #[serde(rename = "_sprite")]
    pub sprite: Option<String>,
    #[serde(rename = "_flipbook")]
    pub flipbook: Option<String>,
    pub description: Option<String>,
    pub coordinates: [i64; 2],
    #[serde(default)]
    pub z_index: i64,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub transforms: Vec<Transform>,
}

/// What a panel element draws. The schema guarantees exactly one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementSource<'a> {
    Sprite(&'a str),
    Flipbook(&'a str),
}

impl Element {
    pub fn source(&self) -> ElementSource<'_> {
        match (&self.sprite, &self.flipbook) {
            (Some(s), _) => ElementSource::Sprite(s),
            (None, Some(f)) => ElementSource::Flipbook(f),
            (None, None) => unreachable!("schema requires _sprite or _flipbook"),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Panel {
    pub id: String,
    pub description: Option<String>,
    pub width: u32,
    pub height: u32,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Transition {
    MoveTo {
        #[serde(rename = "_element")]
        element: String,
        coordinates: [i64; 2],
        start: u64,
        duration: u64,
    },
    SetVisible {
        #[serde(rename = "_element")]
        element: String,
        value: bool,
        at: u64,
    },
    MoveToZ {
        #[serde(rename = "_element")]
        element: String,
        value: i64,
        at: u64,
    },
    AppendTransforms {
        #[serde(rename = "_element")]
        element: String,
        transforms: Vec<Transform>,
        at: u64,
    },
    ResetTransforms {
        #[serde(rename = "_element")]
        element: String,
        at: u64,
    },
}

impl Transition {
    /// Id of the panel element this transition targets.
    pub fn element(&self) -> &str {
        match self {
            Transition::MoveTo { element, .. }
            | Transition::SetVisible { element, .. }
            | Transition::MoveToZ { element, .. }
            | Transition::AppendTransforms { element, .. }
            | Transition::ResetTransforms { element, .. } => element,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Timeline {
    pub id: String,
    pub description: Option<String>,
    #[serde(rename = "_panel")]
    pub panel: String,
    pub fps: u32,
    pub duration: u64,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    #[serde(rename = "_panel")]
    pub panel: Option<String>,
    #[serde(rename = "_timeline")]
    pub timeline: Option<String>,
    pub coordinates: [i64; 2],
    #[serde(default)]
    pub z_index: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Scene {
    pub id: String,
    pub description: Option<String>,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
}
