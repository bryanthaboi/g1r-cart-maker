//! Index feeds on top of `cartcore::index`: fetch, cache the raw body for a
//! day, and keep serving the last good copy when the network is gone.

use crate::http::{hex, Client, HttpError};
use cartcore::index::{parse_feed, resolve_source, Index, IndexSource, CACHE_TTL};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum FeedError {
    #[error("{0}")]
    Source(String),
    #[error("{0}")]
    Fetch(String),
    #[error("{0}")]
    Parse(String),
    #[error("no cached copy of {0}")]
    NotCached(String),
    #[error("{0}")]
    Io(String),
}

/// The raw body plus everything the UI needs to caption where it came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cached {
    pub feed: String,
    pub fetched_at: i64,
    pub from_fallback: bool,
    pub body: String,
}

#[derive(Debug)]
pub struct Fetched {
    pub index: Index,
    pub source: IndexSource,
    pub fetched_at: i64,
    pub stale: bool,
    pub from_fallback: bool,
    pub from_cache: bool,
}

#[derive(Debug, Clone)]
pub struct Thumbnail {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub from_cache: bool,
}

pub fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or(0)
}

fn key(text: &str) -> String {
    hex(&Sha256::digest(text.as_bytes()))
}

/// A feed cache rooted at one directory. Nothing outside it is ever written.
pub struct FeedCache {
    dir: PathBuf,
}

impl FeedCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        FeedCache { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn feed_path(&self, source: &IndexSource) -> PathBuf {
        self.dir
            .join("feeds")
            .join(format!("{}.json", key(&source.feed)))
    }

    fn thumb_path(&self, url: &str) -> PathBuf {
        self.dir.join("thumbs").join(key(url))
    }

    fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), FeedError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|problem| FeedError::Io(problem.to_string()))?;
        }
        let mut name = path.as_os_str().to_os_string();
        name.push(".part");
        let part = PathBuf::from(name);
        fs::write(&part, bytes).map_err(|problem| FeedError::Io(problem.to_string()))?;
        fs::rename(&part, path).map_err(|problem| {
            let _ = fs::remove_file(&part);
            FeedError::Io(problem.to_string())
        })
    }

    pub fn read_cached(&self, source: &IndexSource) -> Option<Cached> {
        let body = fs::read_to_string(self.feed_path(source)).ok()?;
        serde_json::from_str(&body).ok()
    }

    pub fn write_cached(&self, source: &IndexSource, record: &Cached) -> Result<(), FeedError> {
        let encoded =
            serde_json::to_vec(record).map_err(|problem| FeedError::Io(problem.to_string()))?;
        Self::write_atomic(&self.feed_path(source), &encoded)
    }

    /// Forgets one source, so the next load has to go out to the network.
    pub fn forget(&self, source: &IndexSource) {
        let _ = fs::remove_file(self.feed_path(source));
    }

    /// Fetch or serve the feed for `input`. `refresh` ignores a fresh cache;
    /// `offline` never touches the network.
    pub fn load(
        &self,
        client: &Client,
        input: &str,
        refresh: bool,
        offline: bool,
    ) -> Result<Fetched, FeedError> {
        let source = resolve_source(input).map_err(FeedError::Source)?;
        self.load_source(client, source, refresh, offline)
    }

    /// `load` against a source the caller already resolved.
    pub fn load_source(
        &self,
        client: &Client,
        source: IndexSource,
        refresh: bool,
        offline: bool,
    ) -> Result<Fetched, FeedError> {
        let now = now_seconds();
        let cached = self.read_cached(&source);
        let fresh = cached
            .as_ref()
            .map(|hit| now - hit.fetched_at < CACHE_TTL && now >= hit.fetched_at)
            .unwrap_or(false);

        if !refresh && fresh {
            if let Some(hit) = &cached {
                if let Ok(index) = parse_feed(&hit.body) {
                    return Ok(Fetched {
                        index,
                        source,
                        fetched_at: hit.fetched_at,
                        stale: false,
                        from_fallback: hit.from_fallback,
                        from_cache: true,
                    });
                }
            }
        }

        if offline {
            return match cached {
                Some(hit) => {
                    let index = parse_feed(&hit.body).map_err(FeedError::Parse)?;
                    Ok(Fetched {
                        index,
                        source,
                        fetched_at: hit.fetched_at,
                        stale: !fresh,
                        from_fallback: hit.from_fallback,
                        from_cache: true,
                    })
                }
                None => Err(FeedError::NotCached(source.feed)),
            };
        }

        // The raw.githubusercontent mirror is a fallback for a dead Pages feed,
        // never a second opinion on one that answered.
        let mut trouble: Option<HttpError> = None;
        let mut got: Option<(String, bool)> = None;
        match client.get_text(&source.feed, None, false) {
            Ok(body) => got = Some((body, false)),
            Err(problem) => trouble = Some(problem),
        }
        if got.is_none() {
            if let Some(fallback) = &source.fallback {
                if let Ok(body) = client.get_text(fallback, None, false) {
                    got = Some((body, true));
                    trouble = None;
                }
            }
        }

        let (body, from_fallback) = match got {
            Some(got) => got,
            None => {
                let problem = trouble
                    .map(|problem| problem.to_string())
                    .unwrap_or_else(|| source.feed.clone());
                return match cached {
                    Some(hit) => {
                        let index = parse_feed(&hit.body).map_err(FeedError::Parse)?;
                        Ok(Fetched {
                            index,
                            source,
                            fetched_at: hit.fetched_at,
                            stale: true,
                            from_fallback: hit.from_fallback,
                            from_cache: true,
                        })
                    }
                    None => Err(FeedError::Fetch(problem)),
                };
            }
        };

        let index = parse_feed(&body).map_err(FeedError::Parse)?;
        let record = Cached {
            feed: source.feed.clone(),
            fetched_at: now,
            from_fallback,
            body,
        };
        let encoded =
            serde_json::to_vec(&record).map_err(|problem| FeedError::Io(problem.to_string()))?;
        Self::write_atomic(&self.feed_path(&source), &encoded)?;
        Ok(Fetched {
            index,
            source,
            fetched_at: now,
            stale: false,
            from_fallback,
            from_cache: false,
        })
    }

    /// Thumbnails are cached by URL hash and never expire. A failure here is the
    /// caller's to swallow; one bad image must not sink a listing.
    pub fn thumbnail(&self, client: &Client, url: &str) -> Result<Thumbnail, FeedError> {
        let path = self.thumb_path(url);
        let kind_path = path.with_extension("type");
        if let Ok(bytes) = fs::read(&path) {
            let content_type = fs::read_to_string(&kind_path)
                .unwrap_or_else(|_| "application/octet-stream".to_string());
            return Ok(Thumbnail {
                bytes,
                content_type,
                from_cache: true,
            });
        }
        let (bytes, content_type) = client
            .get_bytes(url, false)
            .map_err(|problem| FeedError::Fetch(problem.to_string()))?;
        Self::write_atomic(&path, &bytes)?;
        Self::write_atomic(&kind_path, content_type.as_bytes())?;
        Ok(Thumbnail {
            bytes,
            content_type,
            from_cache: false,
        })
    }
}
