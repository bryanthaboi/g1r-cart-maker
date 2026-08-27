//! cartkit's HTTP manners: one user agent, a floor between requests, three
//! attempts with its backoff ladder, and a hard split between "the API said no"
//! and "the API could not be reached".

use md5::Md5;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MIN_INTERVAL: f64 = 0.4;
pub const MAX_BACKOFF: f64 = 30.0;
pub const ATTEMPTS: u32 = 3;
pub const RATE_LIMIT_MESSAGE: &str =
    "GitHub rate limit reached. Requests use gh's own credential when it is signed in, \
     so run gh auth login (or gh auth status to check) and try again.";

const CHUNK: usize = 262_144;
const TOKEN_VARS: [&str; 3] = ["CARTKIT_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"];

/// A shared stop switch a UI can flip while a download is running.
pub type CancelFlag = Arc<AtomicBool>;

pub fn cancel_flag() -> CancelFlag {
    Arc::new(AtomicBool::new(false))
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// The API answered and said the thing does not exist. This is a finding.
    #[error("{0}")]
    NotFound(String),
    /// The API could not be reached or spoke nonsense. This is only ever a note.
    #[error("{0}")]
    Unreachable(String),
    #[error("cancelled")]
    Cancelled,
    #[error("{url}: response is larger than the {limit} byte cap")]
    TooLarge { url: String, limit: u64 },
    #[error("{0}")]
    Io(String),
}

impl HttpError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, HttpError::NotFound(_))
    }
}

/// Token discovery in cartkit's order; an explicitly passed token wins.
pub fn github_token(explicit: Option<&str>) -> Option<String> {
    if let Some(value) = explicit {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    for name in TOKEN_VARS {
        if let Ok(value) = std::env::var(name) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// The wait after a retriable HTTP status: Retry-After wins, capped either way.
pub fn backoff_delay(attempt: u32, retry_after: Option<f64>) -> f64 {
    match retry_after {
        Some(seconds) if seconds.is_finite() => seconds.min(MAX_BACKOFF),
        _ => (2f64.powi(attempt as i32) * 2.0).min(MAX_BACKOFF),
    }
}

/// The wait after a transport failure, which carries no Retry-After.
pub fn transport_delay(attempt: u32) -> f64 {
    2f64.powi(attempt as i32).min(MAX_BACKOFF)
}

fn reason(code: u16) -> &'static str {
    ureq::http::StatusCode::from_u16(code)
        .ok()
        .and_then(|status| status.canonical_reason())
        .unwrap_or("Unknown Error")
}

/// Python's `urllib.parse.quote` with its default safe set, so a tag with `+`
/// build metadata reaches GitHub as `%2B`.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-' | '~' | '/') {
            out.push(ch);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

/// What a streamed download produced. Both digests are always computed.
#[derive(Debug, Clone)]
pub struct Download {
    pub bytes: u64,
    pub sha256: String,
    pub md5: String,
    pub path: Option<PathBuf>,
}

type Progress<'a> = &'a (dyn Fn(u64, Option<u64>) + Send + Sync);

#[derive(Default)]
pub struct DownloadOpts<'a> {
    /// Where the bytes land. `None` hashes the stream and keeps nothing.
    pub dest: Option<&'a Path>,
    pub max_bytes: Option<u64>,
    pub cancel: Option<&'a CancelFlag>,
    pub progress: Option<Progress<'a>>,
    pub authed: bool,
}

pub struct Client {
    agent: ureq::Agent,
    token: Option<String>,
    last: Mutex<Option<Instant>>,
    scale: f64,
    timeout: Duration,
    download_timeout: Duration,
    api_base: String,
    gamebanana_base: String,
}

impl Default for Client {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Client {
    pub fn new(token: Option<&str>) -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .user_agent(cartcore::schema::USER_AGENT)
            .build();
        Client {
            agent: ureq::Agent::new_with_config(config),
            token: github_token(token),
            last: Mutex::new(None),
            scale: 1.0,
            timeout: Duration::from_secs(30),
            download_timeout: Duration::from_secs(120),
            api_base: "https://api.github.com".to_string(),
            gamebanana_base: "https://gamebanana.com".to_string(),
        }
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    pub fn gamebanana_base(&self) -> &str {
        &self.gamebanana_base
    }

    /// Points the GitHub API somewhere else. Only a test ever moves it.
    pub fn set_api_base(&mut self, base: &str) {
        self.api_base = base.trim_end_matches('/').to_string();
    }

    /// Points the GameBanana API somewhere else. Only a test ever moves it.
    pub fn set_gamebanana_base(&mut self, base: &str) {
        self.gamebanana_base = base.trim_end_matches('/').to_string();
    }

    /// Multiplies every wait. Only a test ever moves this off 1.0.
    pub fn set_wait_scale(&mut self, scale: f64) {
        self.scale = scale.max(0.0);
    }

    fn sleep(&self, seconds: f64) {
        let seconds = seconds * self.scale;
        if seconds > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(seconds));
        }
    }

    fn pause(&self) {
        let last = *self.last.lock().unwrap_or_else(|lock| lock.into_inner());
        if let Some(last) = last {
            let wait = MIN_INTERVAL - last.elapsed().as_secs_f64();
            if wait > 0.0 {
                self.sleep(wait);
            }
        }
    }

    fn mark(&self) {
        let mut last = self.last.lock().unwrap_or_else(|lock| lock.into_inner());
        *last = Some(Instant::now());
    }

    fn open(
        &self,
        url: &str,
        accept: Option<&str>,
        authed: bool,
        timeout: Duration,
    ) -> Result<ureq::http::Response<ureq::Body>, HttpError> {
        for attempt in 0..ATTEMPTS {
            self.pause();
            let mut request = self
                .agent
                .get(url)
                .config()
                .timeout_global(Some(timeout))
                .build();
            if let Some(accept) = accept {
                request = request.header("Accept", accept);
            }
            if authed {
                if let Some(token) = &self.token {
                    request = request.header("Authorization", format!("Bearer {}", token));
                }
            }
            match request.call() {
                Ok(response) => {
                    self.mark();
                    let code = response.status().as_u16();
                    if (200..300).contains(&code) {
                        return Ok(response);
                    }
                    if code == 404 || code == 410 {
                        return Err(HttpError::NotFound(url.to_string()));
                    }
                    if code == 403
                        && header(&response, "x-ratelimit-remaining").as_deref() == Some("0")
                    {
                        return Err(HttpError::Unreachable(RATE_LIMIT_MESSAGE.to_string()));
                    }
                    let retriable = code == 403 || code == 429 || (500..600).contains(&code);
                    if retriable && attempt + 1 < ATTEMPTS {
                        let after = header(&response, "retry-after")
                            .and_then(|raw| raw.trim().parse::<f64>().ok());
                        self.sleep(backoff_delay(attempt, after));
                        continue;
                    }
                    return Err(HttpError::Unreachable(format!(
                        "{}: HTTP {} {}",
                        url,
                        code,
                        reason(code)
                    )));
                }
                Err(problem) => {
                    self.mark();
                    if attempt + 1 < ATTEMPTS {
                        self.sleep(transport_delay(attempt));
                        continue;
                    }
                    return Err(HttpError::Unreachable(format!("{}: {}", url, problem)));
                }
            }
        }
        Err(HttpError::Unreachable(url.to_string()))
    }

    pub fn get_json(
        &self,
        url: &str,
        accept: Option<&str>,
        authed: bool,
    ) -> Result<serde_json::Value, HttpError> {
        let text = self.get_text(url, accept, authed)?;
        serde_json::from_str(&text).map_err(|problem| {
            HttpError::Unreachable(format!("{}: response was not JSON ({})", url, problem))
        })
    }

    pub fn get_text(
        &self,
        url: &str,
        accept: Option<&str>,
        authed: bool,
    ) -> Result<String, HttpError> {
        let mut response = self.open(url, accept, authed, self.timeout)?;
        response
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .lossy_utf8(true)
            .read_to_string()
            .map_err(|problem| HttpError::Unreachable(format!("{}: {}", url, problem)))
    }

    pub fn get_bytes(&self, url: &str, authed: bool) -> Result<(Vec<u8>, String), HttpError> {
        let mut response = self.open(url, None, authed, self.timeout)?;
        let kind = header(&response, "content-type")
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let body = response
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()
            .map_err(|problem| HttpError::Unreachable(format!("{}: {}", url, problem)))?;
        Ok((body, kind))
    }

    /// Streams a URL, hashing as it goes. A failure or a cancel leaves no partial file.
    pub fn download(&self, url: &str, opts: &DownloadOpts) -> Result<Download, HttpError> {
        let mut response = self.open(url, None, opts.authed, self.download_timeout)?;
        let total = response.body().content_length();
        let mut reader = response.body_mut().as_reader();

        let part = opts.dest.map(|dest| {
            let mut name = dest.as_os_str().to_os_string();
            name.push(".part");
            PathBuf::from(name)
        });
        if let (Some(dest), Some(part)) = (opts.dest, part.as_ref()) {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|problem| HttpError::Io(problem.to_string()))?;
            }
            let _ = fs::remove_file(part);
        }
        let mut sink = match part.as_ref() {
            Some(part) => {
                Some(File::create(part).map_err(|problem| HttpError::Io(problem.to_string()))?)
            }
            None => None,
        };

        let mut sha = Sha256::new();
        let mut md5 = Md5::new();
        let mut done: u64 = 0;
        let mut buffer = vec![0u8; CHUNK];
        let outcome = loop {
            if let Some(cancel) = opts.cancel {
                if cancel.load(Ordering::Relaxed) {
                    break Err(HttpError::Cancelled);
                }
            }
            let read = match reader.read(&mut buffer) {
                Ok(0) => break Ok(()),
                Ok(read) => read,
                Err(problem) => break Err(HttpError::Unreachable(format!("{}: {}", url, problem))),
            };
            done += read as u64;
            if let Some(limit) = opts.max_bytes {
                if done > limit {
                    break Err(HttpError::TooLarge {
                        url: url.to_string(),
                        limit,
                    });
                }
            }
            sha.update(&buffer[..read]);
            md5.update(&buffer[..read]);
            if let Some(sink) = sink.as_mut() {
                if let Err(problem) = sink.write_all(&buffer[..read]) {
                    break Err(HttpError::Io(problem.to_string()));
                }
            }
            if let Some(progress) = opts.progress {
                progress(done, total);
            }
        };

        drop(sink);
        if let Err(problem) = outcome {
            if let Some(part) = part.as_ref() {
                let _ = fs::remove_file(part);
            }
            return Err(problem);
        }
        if let (Some(dest), Some(part)) = (opts.dest, part.as_ref()) {
            fs::rename(part, dest).map_err(|problem| {
                let _ = fs::remove_file(part);
                HttpError::Io(problem.to_string())
            })?;
        }
        Ok(Download {
            bytes: done,
            sha256: hex(&sha.finalize()),
            md5: hex(&md5.finalize()),
            path: opts.dest.map(Path::to_path_buf),
        })
    }
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}
