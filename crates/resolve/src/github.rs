//! Release resolution against the GitHub API, message for message with
//! cartkit's `github_release`, `pick_asset`, `sums_digest` and `resolve_github`.

use crate::http::{quote, Client, DownloadOpts, HttpError};
use serde_json::{json, Map, Value};

pub const ACCEPT: &str = "application/vnd.github+json";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub url: String,
}

/// One downloaded release asset: what landed on disk and what it hashes to.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub tag: String,
    pub asset: String,
    pub bytes: u64,
    pub sha256: String,
    pub md5: String,
    pub published_sha256: Option<String>,
    pub path: Option<std::path::PathBuf>,
}

/// One resolved pin: the tag that answered, the archive on it and its hash.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Resolved {
    pub tag: String,
    pub asset: String,
    pub size: u64,
    pub sha256: Option<String>,
    pub how: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReleaseSummary {
    pub tag: String,
    pub name: Option<String>,
    pub published_at: Option<String>,
    pub prerelease: bool,
    pub assets: Vec<Asset>,
}

fn base(client: &Client) -> &str {
    client.api_base()
}

/// `v{version}` first, then the bare version, exactly as the game looks them up.
pub fn github_release(
    client: &Client,
    slug: &str,
    version: &str,
) -> Result<(Value, String), HttpError> {
    let tags = [format!("v{}", version), version.to_string()];
    for tag in &tags {
        let url = format!(
            "{}/repos/{}/releases/tags/{}",
            base(client),
            slug,
            quote(tag)
        );
        match client.get_json(&url, Some(ACCEPT), true) {
            Ok(release) => return Ok((release, tag.clone())),
            Err(problem) if problem.is_not_found() => continue,
            Err(problem) => return Err(problem),
        }
    }
    Err(HttpError::NotFound(format!(
        "{} has no release tagged {}",
        slug,
        tags.join(" or ")
    )))
}

fn assets_of(release: &Value) -> Vec<&Map<String, Value>> {
    release
        .get("assets")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_object).collect())
        .unwrap_or_default()
}

fn named(entry: &Map<String, Value>) -> Option<&str> {
    entry
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
}

fn as_asset(entry: &Map<String, Value>, name: &str) -> Asset {
    Asset {
        name: name.to_string(),
        size: entry.get("size").and_then(Value::as_u64).unwrap_or(0),
        url: entry
            .get("browser_download_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}

fn tag_of(release: &Value) -> String {
    match release.get("tag_name").and_then(Value::as_str) {
        Some(tag) => tag.to_string(),
        None => "None".to_string(),
    }
}

/// The engine loads `{mod_id}-{version}.zip`; a lone .zip is accepted instead.
pub fn pick_asset(release: &Value, mod_id: &str, version: &str) -> Result<Asset, HttpError> {
    let entries = assets_of(release);
    let wanted = format!("{}-{}.zip", mod_id, version);
    for entry in &entries {
        if let Some(name) = named(entry) {
            if name == wanted {
                return Ok(as_asset(entry, name));
            }
        }
    }
    let mut zips: Vec<(&&Map<String, Value>, &str)> = Vec::new();
    for entry in &entries {
        if let Some(name) = named(entry) {
            if name.to_lowercase().ends_with(".zip") {
                zips.push((entry, name));
            }
        }
    }
    if zips.len() == 1 {
        return Ok(as_asset(zips[0].0, zips[0].1));
    }
    if zips.is_empty() {
        return Err(HttpError::NotFound(format!(
            "release {} has no .zip asset; publish the mod archive on the release",
            tag_of(release)
        )));
    }
    let mut names: Vec<&str> = zips.iter().map(|(_, name)| *name).collect();
    names.sort_unstable();
    Err(HttpError::NotFound(format!(
        "release {} has {} .zip assets ({}); the game picks {}, so name the mod archive that way",
        tag_of(release),
        zips.len(),
        names.join(", "),
        wanted
    )))
}

fn basename(text: &str) -> &str {
    match text.rfind('/') {
        Some(at) => &text[at + 1..],
        None => text,
    }
}

/// The published sha256 for one asset, if the release ships a sums file at all.
pub fn sums_digest(
    client: &Client,
    release: &Value,
    asset_name: &str,
) -> Result<Option<String>, HttpError> {
    for entry in assets_of(release) {
        if entry.get("name").and_then(Value::as_str) != Some("sha256sums.txt") {
            continue;
        }
        let url = entry
            .get("browser_download_url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let body = client.get_text(url, None, false)?;
        for line in body.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 && basename(parts[1].trim_start_matches('*')) == asset_name {
                return Ok(Some(parts[0].to_lowercase()));
            }
        }
    }
    Ok(None)
}

/// The whole pin story for one GitHub mod. `download` off keeps the hash unknown
/// rather than pulling the archive.
pub fn resolve_github(
    client: &Client,
    slug: &str,
    version: &str,
    mod_id: &str,
    download: bool,
) -> Result<Resolved, HttpError> {
    resolve_github_watched(
        client,
        slug,
        version,
        mod_id,
        download,
        &DownloadOpts::default(),
    )
}

/// `resolve_github` with a cancel flag and a progress callback attached to the
/// download half; `dest` on the options keeps the archive.
/// Put a release asset on disk and return its digests.
///
/// Pin resolution stops at `sha256sums.txt` because a published digest answers
/// the question without a download; a caller that needs the archive itself has
/// to ask for it, so this always fetches.
pub fn fetch_asset(
    client: &Client,
    slug: &str,
    version: &str,
    mod_id: &str,
    opts: &DownloadOpts,
) -> Result<Fetched, HttpError> {
    let (release, tag) = github_release(client, slug, version)?;
    let asset = pick_asset(&release, mod_id, version)?;
    let published = sums_digest(client, &release, &asset.name)?;
    let got = client.download(&asset.url, opts)?;
    Ok(Fetched {
        tag,
        asset: asset.name,
        bytes: got.bytes,
        sha256: got.sha256,
        md5: got.md5,
        published_sha256: published,
        path: got.path,
    })
}

pub fn resolve_github_watched(
    client: &Client,
    slug: &str,
    version: &str,
    mod_id: &str,
    download: bool,
    opts: &DownloadOpts,
) -> Result<Resolved, HttpError> {
    let (release, tag) = github_release(client, slug, version)?;
    let asset = pick_asset(&release, mod_id, version)?;
    let digest = sums_digest(client, &release, &asset.name)?;
    if let Some(digest) = digest {
        return Ok(Resolved {
            tag,
            asset: asset.name,
            size: asset.size,
            sha256: Some(digest),
            how: "sha256sums.txt".to_string(),
        });
    }
    if !download {
        return Ok(Resolved {
            tag,
            asset: asset.name,
            size: asset.size,
            sha256: None,
            how: "not published".to_string(),
        });
    }
    let got = client.download(&asset.url, opts)?;
    Ok(Resolved {
        tag,
        asset: asset.name,
        size: asset.size,
        sha256: Some(got.sha256),
        how: format!("downloading {} bytes", got.bytes),
    })
}

/// Every release on a repo, newest first, for the "pick a tag" list.
pub fn releases(client: &Client, slug: &str) -> Result<Vec<ReleaseSummary>, HttpError> {
    const PER_PAGE: usize = 100;
    const MAX_PAGES: usize = 10;
    let mut out = Vec::new();
    for page in 1..=MAX_PAGES {
        let url = format!(
            "{}/repos/{}/releases?per_page={}&page={}",
            base(client),
            slug,
            PER_PAGE,
            page
        );
        let body = client.get_json(&url, Some(ACCEPT), true)?;
        let items = match body.as_array() {
            Some(items) => items,
            None => break,
        };
        let count = items.len();
        for item in items {
            let tag = match item.get("tag_name").and_then(Value::as_str) {
                Some(tag) => tag.to_string(),
                None => continue,
            };
            let assets = assets_of(item)
                .into_iter()
                .filter_map(|entry| named(entry).map(|name| as_asset(entry, name)))
                .collect();
            out.push(ReleaseSummary {
                tag,
                name: item.get("name").and_then(Value::as_str).map(str::to_string),
                published_at: item
                    .get("published_at")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                prerelease: item
                    .get("prerelease")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                assets,
            });
        }
        if count < PER_PAGE {
            break;
        }
    }
    Ok(out)
}

/// cartkit's `pin_github`: the cart entry plus the one-line note the CLI prints.
pub fn pin_github(
    client: &Client,
    slug: &str,
    version: &str,
    id: Option<&str>,
    options: &Map<String, Value>,
) -> Result<(Value, String), HttpError> {
    let mod_id = match id {
        Some(id) => id.to_string(),
        None => cartcore::spec::derive_id(slug.split('/').nth(1).unwrap_or(slug)),
    };
    let found = resolve_github(client, slug, version, &mod_id, true)?;
    let sha256 = match found.sha256 {
        Some(sha256) => sha256,
        None => {
            return Err(HttpError::NotFound(format!(
                "{} {} publishes no hash for {}",
                slug, found.tag, found.asset
            )))
        }
    };
    let mut entry = Map::new();
    entry.insert("id".into(), json!(mod_id));
    entry.insert("source".into(), json!("github"));
    entry.insert("repo".into(), json!(slug));
    entry.insert("version".into(), json!(version));
    entry.insert("sha256".into(), json!(sha256));
    if !options.is_empty() {
        entry.insert("options".into(), Value::Object(options.clone()));
    }
    let note = format!(
        "{} {} -> {}, sha256 from {}",
        slug, found.tag, found.asset, found.how
    );
    Ok((Value::Object(entry), note))
}

#[cfg(test)]
mod live {
    /// Fetches the real quality_of_life release, whose sums file used to make
    /// the download be skipped. Ignored by default; run with
    /// `cargo test -p resolve -- --ignored live_fetch`.
    #[test]
    #[ignore]
    fn live_fetch_lands_on_disk() {
        let dir = std::env::temp_dir().join("g1r-live-fetch");
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("qol.zip");
        let _ = std::fs::remove_file(&dest);
        let client = crate::http::Client::new(None);
        let opts = crate::http::DownloadOpts {
            dest: Some(&dest),
            max_bytes: None,
            cancel: None,
            progress: None,
            authed: false,
        };
        let got = super::fetch_asset(
            &client,
            "unxpected-uxp/pokemon-gen1-recomp-mod-qol",
            "1.3.0",
            "quality_of_life",
            &opts,
        )
        .expect("the release must resolve");
        assert!(dest.is_file(), "the archive must be on disk");
        assert_eq!(got.asset, "quality_of_life-1.3.0.zip");
        assert_eq!(got.published_sha256.as_deref(), Some(got.sha256.as_str()));
        println!("live fetch: {} bytes, sha256 {}", got.bytes, got.sha256);
    }
}
