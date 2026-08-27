//! Label templates and label art writing. The six shipped templates are the
//! engine's own cartridge labels, 500x441 each.

use crate::error::{AppError, AppResult};
use crate::project::{decode_png_data_url, png_data_url};
use cartcore::labeldoc::{check_export, ExportCheck};
use serde::Serialize;
use std::path::Path;

macro_rules! template {
    ($id:expr, $name:expr, $file:expr) => {
        (
            $id,
            $name,
            include_bytes!(concat!("../../assets/labels/", $file)) as &'static [u8],
        )
    };
}

const TEMPLATES: [(&str, &str, &[u8]); 6] = [
    template!("red", "Red", "red.png"),
    template!("blue", "Blue", "blue.png"),
    template!("yellow", "Yellow", "yellow.png"),
    template!("gold", "Gold", "gold.png"),
    template!("silver", "Silver", "silver.png"),
    template!("crystal", "Crystal", "crystal.png"),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelTemplate {
    pub id: String,
    pub name: String,
    pub base: Option<String>,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

pub fn templates() -> Vec<LabelTemplate> {
    TEMPLATES
        .iter()
        .map(|(id, name, bytes)| {
            let (width, height) = cartcore::labelart::png_dimensions(bytes).unwrap_or((500, 441));
            LabelTemplate {
                id: (*id).to_string(),
                name: (*name).to_string(),
                base: Some((*id).to_string()),
                width,
                height,
                data_url: png_data_url(bytes),
            }
        })
        .collect()
}

pub fn check(data_url: &str, label_path: &str) -> AppResult<ExportCheck> {
    let bytes = decode_png_data_url(data_url)?;
    Ok(check_export(&bytes, label_path))
}

/// Never writes art the manifest would then reject.
pub fn write_png(dir: &Path, label_path: &str, data_url: &str) -> AppResult<ExportCheck> {
    let bytes = decode_png_data_url(data_url)?;
    let check = check_export(&bytes, label_path);
    if !check.ok {
        return Err(
            AppError::invalid("that label art is not something the cart can carry")
                .with_detail(check.problems.join("\n")),
        );
    }
    let path = dir.join(label_path);
    if !path.starts_with(dir) {
        return Err(AppError::invalid(
            "the label path must stay inside the cart",
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::settings::write_atomic(&path, &bytes)?;
    Ok(check)
}

pub fn placeholder(shell: &str) -> AppResult<String> {
    if !cartcore::schema::shell_re().is_match(shell) {
        return Err(AppError::invalid("a shell colour looks like #8b1a1a"));
    }
    Ok(png_data_url(&cartcore::labelart::label_art(shell)))
}

/// Read an image off disk as a data URL.
///
/// The asset protocol would need a filesystem scope and a wider CSP; the bytes
/// come back through a command instead, so the window is never granted a way to
/// read files on its own.
pub fn read_image_data_url(path: &Path) -> AppResult<String> {
    const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_IMAGE_BYTES {
        return Err(AppError::invalid(format!(
            "that image is {} MB; the limit is 32 MB",
            meta.len() / (1024 * 1024)
        )));
    }
    let bytes = std::fs::read(path)?;
    let mime = match sniff(&bytes) {
        Some(mime) => mime,
        None => return Err(AppError::invalid("that file is not a PNG, JPEG or WebP")),
    };
    use base64::Engine as _;
    Ok(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
}

/// Trust the bytes, not the extension.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}
