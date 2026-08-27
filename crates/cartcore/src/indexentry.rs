//! `index-entry.json` beside cart.json: the fields the community index shows
//! that `cart.json` has no room for.
//!
//! They cannot live in the cart itself. `cart.json` has a closed key set, so a
//! `thumbnail` or `tags` key there is a CK001 unknown-field warning, and pack
//! runs strict, so the bundle would be refused. This sidecar is never packed
//! and never published; it only feeds the readiness checks and prefills the
//! index submission.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;

pub const ENTRY_FILE: &str = "index-entry.json";

pub const MAX_TAGS: usize = 12;
pub const MAX_TAG: usize = 24;
pub const MAX_URL: usize = 512;
pub const MAX_LICENSE: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct IndexEntry {
    pub thumbnail: Option<String>,
    pub description_url: Option<String>,
    pub license: Option<String>,
    pub tags: Vec<String>,
    /// `None` leaves the index's own default alone.
    pub automatic_version_check: Option<bool>,
    pub fixed_release_tag: Option<String>,
}

fn clean(value: Option<String>, limit: usize) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(limit).collect())
}

impl IndexEntry {
    /// Trim, drop blanks, cap lengths and de-duplicate tags.
    pub fn normalized(mut self) -> Self {
        self.thumbnail = clean(self.thumbnail, MAX_URL);
        self.description_url = clean(self.description_url, MAX_URL);
        self.license = clean(self.license, MAX_LICENSE);
        self.fixed_release_tag = clean(self.fixed_release_tag, MAX_LICENSE);
        let mut tags: Vec<String> = Vec::new();
        for tag in std::mem::take(&mut self.tags) {
            let tag: String = tag.trim().chars().take(MAX_TAG).collect();
            if tag.is_empty() || tags.iter().any(|seen| seen.eq_ignore_ascii_case(&tag)) {
                continue;
            }
            tags.push(tag);
            if tags.len() == MAX_TAGS {
                break;
            }
        }
        self.tags = tags;
        self
    }

    pub fn is_empty(&self) -> bool {
        *self == IndexEntry::default()
    }

    /// What a cart carries directly, for a cart that accepted the CK001 warning.
    pub fn from_cart(cart: &crate::Cart) -> Self {
        let text = |key: &str| cart.get(key).and_then(Value::as_str).map(str::to_string);
        IndexEntry {
            thumbnail: text("thumbnail"),
            description_url: text("description_url"),
            license: text("license"),
            tags: cart
                .get("tags")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            automatic_version_check: cart.get("automatic_version_check").and_then(Value::as_bool),
            fixed_release_tag: text("fixed_release_tag"),
        }
        .normalized()
    }

    /// The sidecar wins field by field; anything it leaves unset falls back to
    /// what the cart carries.
    pub fn over(self, cart: &crate::Cart) -> Self {
        let base = IndexEntry::from_cart(cart);
        IndexEntry {
            thumbnail: self.thumbnail.or(base.thumbnail),
            description_url: self.description_url.or(base.description_url),
            license: self.license.or(base.license),
            tags: if self.tags.is_empty() {
                base.tags
            } else {
                self.tags
            },
            automatic_version_check: self
                .automatic_version_check
                .or(base.automatic_version_check),
            fixed_release_tag: self.fixed_release_tag.or(base.fixed_release_tag),
        }
    }
}

/// A missing or unreadable sidecar is an empty entry, never an error: it is
/// optional metadata, and a cart without one is simply not filled in yet.
pub fn read(dir: &Path) -> IndexEntry {
    let path = dir.join(ENTRY_FILE);
    let Ok(text) = fs::read_to_string(path) else {
        return IndexEntry::default();
    };
    serde_json::from_str::<IndexEntry>(&text)
        .map(IndexEntry::normalized)
        .unwrap_or_default()
}

pub fn write(dir: &Path, entry: &IndexEntry) -> io::Result<()> {
    let path = dir.join(ENTRY_FILE);
    let entry = entry.clone().normalized();
    if entry.is_empty() {
        return match fs::remove_file(&path) {
            Err(problem) if problem.kind() != io::ErrorKind::NotFound => Err(problem),
            _ => Ok(()),
        };
    }
    let mut text = serde_json::to_string_pretty(&entry)
        .map_err(|problem| io::Error::new(io::ErrorKind::InvalidData, problem))?;
    text.push('\n');
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dir() -> tempdir::TempDir {
        tempdir::TempDir::new("entry").unwrap()
    }

    #[test]
    fn a_missing_sidecar_is_empty_not_an_error() {
        assert!(read(dir().path()).is_empty());
    }

    #[test]
    fn a_round_trip_keeps_every_field() {
        let temp = dir();
        let entry = IndexEntry {
            thumbnail: Some("https://example.test/t.png".into()),
            description_url: Some("https://example.test/#readme".into()),
            license: Some("MIT".into()),
            tags: vec!["hard".into(), "randomizer".into()],
            automatic_version_check: Some(false),
            fixed_release_tag: Some("v1.2.3".into()),
        };
        write(temp.path(), &entry).unwrap();
        assert_eq!(read(temp.path()), entry);
    }

    #[test]
    fn tags_are_trimmed_deduped_and_capped() {
        let entry = IndexEntry {
            tags: (0..30).map(|n| format!("  tag{}  ", n % 3)).collect(),
            ..IndexEntry::default()
        }
        .normalized();
        assert_eq!(entry.tags, vec!["tag0", "tag1", "tag2"]);
    }

    #[test]
    fn a_blank_field_is_dropped_rather_than_stored_empty() {
        let entry = IndexEntry {
            thumbnail: Some("   ".into()),
            license: Some(String::new()),
            ..IndexEntry::default()
        }
        .normalized();
        assert!(entry.thumbnail.is_none());
        assert!(entry.license.is_none());
        assert!(entry.is_empty());
    }

    #[test]
    fn emptying_the_entry_removes_the_file() {
        let temp = dir();
        write(
            temp.path(),
            &IndexEntry {
                license: Some("MIT".into()),
                ..IndexEntry::default()
            },
        )
        .unwrap();
        assert!(temp.path().join(ENTRY_FILE).is_file());
        write(temp.path(), &IndexEntry::default()).unwrap();
        assert!(!temp.path().join(ENTRY_FILE).exists());
    }

    #[test]
    fn the_sidecar_wins_but_the_cart_fills_the_gaps() {
        let cart = json!({
            "thumbnail": "https://cart.test/t.png",
            "license": "Apache-2.0",
            "tags": ["from-cart"],
        });
        let cart = cart.as_object().unwrap().clone();
        let merged = IndexEntry {
            license: Some("MIT".into()),
            ..IndexEntry::default()
        }
        .over(&cart);
        assert_eq!(merged.license.as_deref(), Some("MIT"));
        assert_eq!(merged.thumbnail.as_deref(), Some("https://cart.test/t.png"));
        assert_eq!(merged.tags, vec!["from-cart"]);
    }

    #[test]
    fn a_corrupt_sidecar_reads_as_empty_rather_than_failing_the_screen() {
        let temp = dir();
        std::fs::write(temp.path().join(ENTRY_FILE), "{ not json").unwrap();
        assert!(read(temp.path()).is_empty());
    }
}
