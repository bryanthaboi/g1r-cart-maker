//! The procedural placeholder label a fresh cart carries, byte-identical to
//! cartkit's `label_art`. Deflate must be zlib level 9 for that to hold.

use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;

pub const PLACEHOLDER_SIZE: u32 = 96;

pub fn shell_rgb(shell: &str) -> Option<(i32, i32, i32)> {
    let value = shell.strip_prefix('#').unwrap_or(shell);
    if value.len() != 6 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let component = |start: usize| i32::from_str_radix(&value[start..start + 2], 16).ok();
    Some((component(0)?, component(2)?, component(4)?))
}

fn mix(colour: (i32, i32, i32), target: (i32, i32, i32), amount: f64) -> (i32, i32, i32) {
    // Python's round() is half-to-even; the placeholder art depends on it.
    let blend =
        |c: i32, t: i32| (c as f64 + (t as f64 - c as f64) * amount).round_ties_even() as i32;
    (
        blend(colour.0, target.0),
        blend(colour.1, target.1),
        blend(colour.2, target.2),
    )
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(body);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(tag);
    hasher.update(body);
    out.extend_from_slice(&hasher.finalize().to_be_bytes());
}

/// 8-bit truecolour PNG, one filter-0 scanline per row, exactly as cartkit writes it.
pub fn png_bytes(width: u32, height: u32, rows: &[Vec<(i32, i32, i32)>]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((height * (1 + width * 3)) as usize);
    for row in rows {
        raw.push(0u8);
        for pixel in row {
            raw.push(pixel.0 as u8);
            raw.push(pixel.1 as u8);
            raw.push(pixel.2 as u8);
        }
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(9));
    encoder.write_all(&raw).expect("in-memory deflate");
    let idat = encoder.finish().expect("in-memory deflate");

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    out
}

pub fn label_art(shell: &str) -> Vec<u8> {
    let size = PLACEHOLDER_SIZE as i32;
    let face = shell_rgb(shell).unwrap_or((139, 26, 26));
    let edge = mix(face, (0, 0, 0), 0.55);
    let sticker = mix(face, (255, 255, 255), 0.78);
    let ink = mix(face, (0, 0, 0), 0.35);
    let mut rows = Vec::with_capacity(size as usize);
    for y in 0..size {
        let mut row = Vec::with_capacity(size as usize);
        for x in 0..size {
            let inset = x.min(y).min(size - 1 - x).min(size - 1 - y);
            let pixel = if inset < 5 {
                edge
            } else if (12..size - 12).contains(&x) && (16..size - 28).contains(&y) {
                if (x + y) % 16 != 0 {
                    sticker
                } else {
                    ink
                }
            } else {
                face
            };
            row.push(pixel);
        }
        rows.push(row);
    }
    png_bytes(size as u32, size as u32, &rows)
}

/// Width and height out of a PNG header, for validating imported art.
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || bytes[..8] != crate::schema::PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Some((width, height))
}

pub fn is_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..8] == crate::schema::PNG_SIGNATURE
}
