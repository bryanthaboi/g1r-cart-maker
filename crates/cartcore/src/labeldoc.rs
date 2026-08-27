//! The label layer document: `label.layers.json` beside cart.json. The designer
//! edits it, the exported PNG is what the manifest points at.

use crate::labelart::{is_png, png_dimensions};
use crate::schema::{LABEL_MAX_BYTES, LABEL_WARN_BYTES, MAX_LABEL_PATH};
use serde::{Deserialize, Serialize};

pub const DOC_FILE: &str = "label.layers.json";
pub const DOC_SCHEMA: u32 = 1;
/// Every shipped template is 500x441; that is the cart label canvas.
pub const CANVAS_WIDTH: u32 = 500;
pub const CANVAS_HEIGHT: u32 = 441;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    Contain,
    Cover,
    Crop,
    Scale,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LayerBody {
    /// An imported bitmap, stored as a data URL so a project stays self-contained.
    Image {
        source: String,
        fit: FitMode,
        #[serde(default)]
        opacity: Option<f64>,
    },
    Text {
        text: String,
        font: String,
        size: f64,
        colour: String,
        align: TextAlign,
        #[serde(default)]
        weight: Option<String>,
        #[serde(default)]
        letter_spacing: Option<f64>,
        #[serde(default)]
        line_height: Option<f64>,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default)]
        stroke_width: Option<f64>,
    },
    Rect {
        fill: String,
        #[serde(default)]
        radius: Option<f64>,
        #[serde(default)]
        stroke: Option<String>,
        #[serde(default)]
        stroke_width: Option<f64>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    /// Set on layers derived from a template, so a reset can restore them.
    #[serde(default)]
    pub from_template: Option<String>,
    #[serde(flatten)]
    pub body: LayerBody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelDoc {
    pub schema: u32,
    pub width: u32,
    pub height: u32,
    /// Template id (`red`, `gold`, ...) or `blank`.
    pub template: String,
    pub background: String,
    pub layers: Vec<Layer>,
}

impl Default for LabelDoc {
    fn default() -> Self {
        Self {
            schema: DOC_SCHEMA,
            width: CANVAS_WIDTH,
            height: CANVAS_HEIGHT,
            template: "blank".to_string(),
            background: "#ffffff".to_string(),
            layers: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LabelDocError {
    #[error("label document unreadable: {0}")]
    Unreadable(String),
    #[error("label document schema {0} is newer than this app reads ({DOC_SCHEMA})")]
    Newer(u32),
}

pub fn parse_doc(body: &str) -> Result<LabelDoc, LabelDocError> {
    let probe: serde_json::Value = serde_json::from_str(body)
        .map_err(|problem| LabelDocError::Unreadable(problem.to_string()))?;
    let schema = probe
        .get("schema")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1) as u32;
    if schema > DOC_SCHEMA {
        return Err(LabelDocError::Newer(schema));
    }
    serde_json::from_str(body).map_err(|problem| LabelDocError::Unreadable(problem.to_string()))
}

pub fn serialize_doc(doc: &LabelDoc) -> String {
    let mut body = serde_json::to_string_pretty(doc).expect("a label document always serializes");
    body.push('\n');
    body
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCheck {
    pub ok: bool,
    pub bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
}

/// Refuse art the manifest would reject, before it is written.
pub fn check_export(bytes: &[u8], label_path: &str) -> ExportCheck {
    let mut problems = Vec::new();
    let mut warnings = Vec::new();
    if !is_png(bytes) {
        problems.push("the exported file is not a PNG".to_string());
    }
    let size = bytes.len() as u64;
    if size > LABEL_MAX_BYTES {
        problems.push(format!(
            "label art is {} bytes; the manifest caps it at {}",
            size, LABEL_MAX_BYTES
        ));
    } else if size > LABEL_WARN_BYTES {
        warnings.push(format!(
            "label art is {} bytes; a cart label wants a few KB, not a photo",
            size
        ));
    }
    if label_path.chars().count() > MAX_LABEL_PATH {
        problems.push(format!(
            "label path is longer than {} characters",
            MAX_LABEL_PATH
        ));
    }
    if let Some(problem) =
        crate::validate::label_problem(&serde_json::Value::String(label_path.to_string()))
    {
        problems.push(format!("label path {}", problem));
    }
    let dimensions = png_dimensions(bytes);
    ExportCheck {
        ok: problems.is_empty(),
        bytes: size,
        width: dimensions.map(|(w, _)| w),
        height: dimensions.map(|(_, h)| h),
        problems,
        warnings,
    }
}
