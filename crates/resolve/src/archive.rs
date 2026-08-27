//! Mod archives. Every entry name in a downloaded zip is hostile until it has
//! been through `safe_entry`, and every archive is hashed before it is trusted.

use crate::http::hex;
use md5::Md5;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use zip::ZipArchive;

const SYMLINK_MODE: u32 = 0o120_000;
const FILE_TYPE_MASK: u32 = 0o170_000;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("{0}")]
    Io(String),
    #[error("{path} is not a readable zip: {problem}")]
    NotAZip { path: String, problem: String },
    #[error("archive entry {0:?} escapes the destination directory")]
    Unsafe(String),
    #[error("archive entry {0:?} is a symlink; mod archives are plain files only")]
    Symlink(String),
    #[error("archive has {count} entries, past the {limit} entry cap")]
    TooManyEntries { count: usize, limit: usize },
    #[error("archive expands to more than the {limit} byte cap")]
    TooBig { limit: u64 },
    #[error("archive entry {name:?} expands {ratio}x, past the {limit}x cap")]
    Ratio {
        name: String,
        ratio: u64,
        limit: u64,
    },
    #[error("archive entry {0:?} is not in the archive")]
    Missing(String),
    #[error(
        "{path} hashes to {got} but {published} was published; the download does not match the \
         pin and was not kept"
    )]
    HashMismatch {
        path: String,
        got: String,
        published: String,
    },
}

/// Ceilings that turn a zip bomb into an error instead of a full disk.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_total_bytes: u64,
    pub max_entries: usize,
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_total_bytes: 512 * 1024 * 1024,
            max_entries: 10_000,
            max_ratio: 200,
        }
    }
}

/// The one place an archive path becomes a filesystem path. Backslashes,
/// drive letters, absolute roots and `..` are all refusals, never rewrites.
pub fn safe_entry(name: &str) -> Result<PathBuf, ArchiveError> {
    let bad = || ArchiveError::Unsafe(name.to_string());
    if name.is_empty() || name.contains('\\') || name.contains('\0') {
        return Err(bad());
    }
    if name.starts_with('/') {
        return Err(bad());
    }
    let bytes = name.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && (bytes[0] as char).is_ascii_alphabetic() {
        return Err(bad());
    }
    let mut out = PathBuf::new();
    for segment in name.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(bad());
        }
        out.push(segment);
    }
    if out.as_os_str().is_empty() {
        return Err(bad());
    }
    if out
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(bad());
    }
    Ok(out)
}

fn open(path: &Path) -> Result<ZipArchive<BufReader<File>>, ArchiveError> {
    let file = File::open(path).map_err(|problem| ArchiveError::Io(problem.to_string()))?;
    ZipArchive::new(BufReader::new(file)).map_err(|problem| ArchiveError::NotAZip {
        path: path.display().to_string(),
        problem: problem.to_string(),
    })
}

fn is_symlink(mode: Option<u32>) -> bool {
    mode.map(|mode| mode & FILE_TYPE_MASK == SYMLINK_MODE)
        .unwrap_or(false)
}

/// Extracts every entry under `dest`, returning the relative paths written.
pub fn extract_zip(
    archive: &Path,
    dest: &Path,
    limits: &Limits,
) -> Result<Vec<PathBuf>, ArchiveError> {
    let mut zip = open(archive)?;
    if zip.len() > limits.max_entries {
        return Err(ArchiveError::TooManyEntries {
            count: zip.len(),
            limit: limits.max_entries,
        });
    }
    fs::create_dir_all(dest).map_err(|problem| ArchiveError::Io(problem.to_string()))?;

    let mut written = Vec::new();
    let mut total: u64 = 0;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|problem| ArchiveError::NotAZip {
                path: archive.display().to_string(),
                problem: problem.to_string(),
            })?;
        let name = entry.name().to_string();
        if entry.is_symlink() || is_symlink(entry.unix_mode()) {
            return Err(ArchiveError::Symlink(name));
        }
        if entry.is_dir() {
            let relative = safe_entry(name.trim_end_matches('/'))?;
            fs::create_dir_all(dest.join(&relative))
                .map_err(|problem| ArchiveError::Io(problem.to_string()))?;
            continue;
        }
        let relative = safe_entry(&name)?;
        let ratio = entry.size() / entry.compressed_size().max(1);
        if ratio > limits.max_ratio {
            return Err(ArchiveError::Ratio {
                name,
                ratio,
                limit: limits.max_ratio,
            });
        }
        let target = dest.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|problem| ArchiveError::Io(problem.to_string()))?;
        }
        let mut sink =
            File::create(&target).map_err(|problem| ArchiveError::Io(problem.to_string()))?;
        let mut buffer = [0u8; 65_536];
        loop {
            let read = entry
                .read(&mut buffer)
                .map_err(|problem| ArchiveError::Io(problem.to_string()))?;
            if read == 0 {
                break;
            }
            total += read as u64;
            if total > limits.max_total_bytes {
                drop(sink);
                let _ = fs::remove_file(&target);
                return Err(ArchiveError::TooBig {
                    limit: limits.max_total_bytes,
                });
            }
            sink.write_all(&buffer[..read])
                .map_err(|problem| ArchiveError::Io(problem.to_string()))?;
        }
        written.push(relative);
    }
    Ok(written)
}

/// One file out of an archive without unpacking it, under the same path rules.
pub fn read_entry(archive: &Path, name: &str, limit: u64) -> Result<Vec<u8>, ArchiveError> {
    let wanted = safe_entry(name)?;
    let mut zip = open(archive)?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|problem| ArchiveError::NotAZip {
                path: archive.display().to_string(),
                problem: problem.to_string(),
            })?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().to_string();
        let relative = match safe_entry(&entry_name) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        if relative != wanted {
            continue;
        }
        if entry.is_symlink() || is_symlink(entry.unix_mode()) {
            return Err(ArchiveError::Symlink(entry_name));
        }
        let mut out = Vec::new();
        let mut buffer = [0u8; 65_536];
        loop {
            let read = entry
                .read(&mut buffer)
                .map_err(|problem| ArchiveError::Io(problem.to_string()))?;
            if read == 0 {
                break;
            }
            out.extend_from_slice(&buffer[..read]);
            if out.len() as u64 > limit {
                return Err(ArchiveError::TooBig { limit });
            }
        }
        return Ok(out);
    }
    Err(ArchiveError::Missing(name.to_string()))
}

/// Every entry name a zip carries, unfiltered, for a "what is in here" preview.
pub fn list_entries(archive: &Path) -> Result<Vec<String>, ArchiveError> {
    let mut zip = open(archive)?;
    let mut out = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|problem| ArchiveError::NotAZip {
                path: archive.display().to_string(),
                problem: problem.to_string(),
            })?;
        out.push(entry.name().to_string());
    }
    Ok(out)
}

/// Both digests of a file on disk, in one pass.
pub fn file_digests(path: &Path) -> Result<(String, String), ArchiveError> {
    let mut file = File::open(path).map_err(|problem| ArchiveError::Io(problem.to_string()))?;
    let mut sha = Sha256::new();
    let mut md5 = Md5::new();
    let mut buffer = vec![0u8; 262_144];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|problem| ArchiveError::Io(problem.to_string()))?;
        if read == 0 {
            break;
        }
        sha.update(&buffer[..read]);
        md5.update(&buffer[..read]);
    }
    Ok((hex(&sha.finalize()), hex(&md5.finalize())))
}

/// Refuses, loudly, an archive whose hash is not the one the pin published.
pub fn verify(path: &Path, sha256: Option<&str>, md5: Option<&str>) -> Result<(), ArchiveError> {
    let (got_sha, got_md5) = file_digests(path)?;
    if let Some(published) = sha256 {
        if !got_sha.eq_ignore_ascii_case(published) {
            return Err(ArchiveError::HashMismatch {
                path: path.display().to_string(),
                got: got_sha,
                published: published.to_string(),
            });
        }
    }
    if let Some(published) = md5 {
        if !got_md5.eq_ignore_ascii_case(published) {
            return Err(ArchiveError::HashMismatch {
                path: path.display().to_string(),
                got: got_md5,
                published: published.to_string(),
            });
        }
    }
    Ok(())
}

/// Downloads keyed by their own sha256, so re-resolving a pin costs nothing.
pub struct ArchiveCache {
    dir: PathBuf,
}

impl ArchiveCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        ArchiveCache { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path_for(&self, sha256: &str) -> PathBuf {
        let sha256 = sha256.to_lowercase();
        let shard = if sha256.len() >= 2 {
            &sha256[..2]
        } else {
            "00"
        };
        self.dir.join(shard).join(format!("{}.zip", sha256))
    }

    pub fn get(&self, sha256: &str) -> Option<PathBuf> {
        let path = self.path_for(sha256);
        path.is_file().then_some(path)
    }

    /// Moves a freshly downloaded file into the store, verifying it first.
    pub fn store(&self, source: &Path, sha256: &str) -> Result<PathBuf, ArchiveError> {
        verify(source, Some(sha256), None)?;
        let target = self.path_for(sha256);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|problem| ArchiveError::Io(problem.to_string()))?;
        }
        if fs::rename(source, &target).is_err() {
            fs::copy(source, &target).map_err(|problem| ArchiveError::Io(problem.to_string()))?;
            let _ = fs::remove_file(source);
        }
        Ok(target)
    }

    pub fn forget(&self, sha256: &str) {
        let _ = fs::remove_file(self.path_for(sha256));
    }
}
