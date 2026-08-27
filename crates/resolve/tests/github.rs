mod support;

use resolve::github::{github_release, pick_asset, releases, resolve_github, sums_digest};
use serde_json::{json, Value};
use std::sync::Arc;
use support::{Reply, TestServer};

const HELLO_SHA: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
const TAG_V: &str = "/repos/owner/demo/releases/tags/v1.2.3";
const TAG_BARE: &str = "/repos/owner/demo/releases/tags/1.2.3";

fn release(base: &str, tag: &str, names: &[&str]) -> Value {
    let assets: Vec<Value> = names
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "size": 11,
                "browser_download_url": format!("{}/dl/{}", base, name),
            })
        })
        .collect();
    json!({ "tag_name": tag, "assets": assets })
}

#[test]
fn exact_name_asset_wins() {
    let body = release("http://x", "v1.2.3", &["other.zip", "demo-1.2.3.zip"]);
    let picked = pick_asset(&body, "demo", "1.2.3").unwrap();
    assert_eq!(picked.name, "demo-1.2.3.zip");
    assert_eq!(picked.size, 11);
}

#[test]
fn a_lone_zip_is_accepted() {
    let body = release("http://x", "v1.2.3", &["notes.txt", "anything.ZIP"]);
    assert_eq!(
        pick_asset(&body, "demo", "1.2.3").unwrap().name,
        "anything.ZIP"
    );
}

#[test]
fn zero_zips_names_the_fix() {
    let body = release("http://x", "v1.2.3", &["notes.txt"]);
    let problem = pick_asset(&body, "demo", "1.2.3").unwrap_err();
    assert!(problem.is_not_found());
    assert_eq!(
        problem.to_string(),
        "release v1.2.3 has no .zip asset; publish the mod archive on the release"
    );
}

#[test]
fn many_zips_list_sorted_and_name_the_wanted_one() {
    let body = release("http://x", "v1.2.3", &["zeta.zip", "alpha.zip"]);
    let problem = pick_asset(&body, "demo", "1.2.3").unwrap_err();
    assert_eq!(
        problem.to_string(),
        "release v1.2.3 has 2 .zip assets (alpha.zip, zeta.zip); the game picks \
         demo-1.2.3.zip, so name the mod archive that way"
    );
}

#[test]
fn the_v_tag_is_tried_first() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        TAG_V => Reply::ok(release(base, "v1.2.3", &["demo-1.2.3.zip"]).to_string()),
        _ => Reply::status(404),
    });
    let client = support::client(&server);
    let (_, tag) = github_release(&client, "owner/demo", "1.2.3").unwrap();
    assert_eq!(tag, "v1.2.3");
    assert_eq!(server.hits(TAG_BARE), 0);
}

#[test]
fn the_bare_tag_is_the_fallback() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        TAG_BARE => Reply::ok(release(base, "1.2.3", &["demo-1.2.3.zip"]).to_string()),
        _ => Reply::status(404),
    });
    let client = support::client(&server);
    let (_, tag) = github_release(&client, "owner/demo", "1.2.3").unwrap();
    assert_eq!(tag, "1.2.3");
    assert_eq!(server.hits(TAG_V), 1);
}

#[test]
fn neither_tag_is_a_not_found_naming_both() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    let problem = github_release(&client, "owner/demo", "1.2.3").unwrap_err();
    assert!(problem.is_not_found());
    assert_eq!(
        problem.to_string(),
        "owner/demo has no release tagged v1.2.3 or 1.2.3"
    );
}

#[test]
fn sums_file_is_parsed_with_a_star_and_a_directory() {
    let server = TestServer::start_based(|path, _hit, _base| match path {
        "/dl/sha256sums.txt" => Reply::ok("aa11 *sub/dir/demo-1.2.3.zip\nbb22  other.zip\n"),
        _ => Reply::status(404),
    });
    let body = release(
        &server.base,
        "v1.2.3",
        &["demo-1.2.3.zip", "sha256sums.txt"],
    );
    let client = support::client(&server);
    let digest = sums_digest(&client, &body, "demo-1.2.3.zip").unwrap();
    assert_eq!(digest.as_deref(), Some("aa11"));
}

#[test]
fn a_malformed_sums_line_is_ignored() {
    let server = TestServer::start_based(|path, _hit, _base| match path {
        "/dl/sha256sums.txt" => Reply::ok("not a sums line at all\nonlyonefield\n\n"),
        _ => Reply::status(404),
    });
    let body = release(
        &server.base,
        "v1.2.3",
        &["demo-1.2.3.zip", "sha256sums.txt"],
    );
    let client = support::client(&server);
    assert_eq!(sums_digest(&client, &body, "demo-1.2.3.zip").unwrap(), None);
}

#[test]
fn no_sums_file_is_no_digest() {
    let server = TestServer::fixtures(vec![]);
    let body = release(&server.base, "v1.2.3", &["demo-1.2.3.zip"]);
    let client = support::client(&server);
    assert_eq!(sums_digest(&client, &body, "demo-1.2.3.zip").unwrap(), None);
}

#[test]
fn resolve_reads_the_hash_from_the_sums_file() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        TAG_V => {
            Reply::ok(release(base, "v1.2.3", &["demo-1.2.3.zip", "sha256sums.txt"]).to_string())
        }
        "/dl/sha256sums.txt" => Reply::ok(format!("{} demo-1.2.3.zip\n", HELLO_SHA)),
        _ => Reply::status(404),
    });
    let client = support::client(&server);
    let found = resolve_github(&client, "owner/demo", "1.2.3", "demo", true).unwrap();
    assert_eq!(found.tag, "v1.2.3");
    assert_eq!(found.asset, "demo-1.2.3.zip");
    assert_eq!(found.size, 11);
    assert_eq!(found.sha256.as_deref(), Some(HELLO_SHA));
    assert_eq!(found.how, "sha256sums.txt");
    assert_eq!(
        server.hits("/dl/demo-1.2.3.zip"),
        0,
        "no download was needed"
    );
}

#[test]
fn resolve_downloads_when_no_sums_file_is_published() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        TAG_V => Reply::ok(release(base, "v1.2.3", &["demo-1.2.3.zip"]).to_string()),
        "/dl/demo-1.2.3.zip" => Reply::ok("hello world"),
        _ => Reply::status(404),
    });
    let client = support::client(&server);
    let found = resolve_github(&client, "owner/demo", "1.2.3", "demo", true).unwrap();
    assert_eq!(found.sha256.as_deref(), Some(HELLO_SHA));
    assert_eq!(found.how, "downloading 11 bytes");
    assert_eq!(server.hits("/dl/demo-1.2.3.zip"), 1);
}

#[test]
fn no_download_leaves_the_hash_unknown() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        TAG_V => Reply::ok(release(base, "v1.2.3", &["demo-1.2.3.zip"]).to_string()),
        _ => Reply::status(404),
    });
    let client = support::client(&server);
    let found = resolve_github(&client, "owner/demo", "1.2.3", "demo", false).unwrap();
    assert_eq!(found.sha256, None);
    assert_eq!(found.how, "not published");
    assert_eq!(server.hits("/dl/demo-1.2.3.zip"), 0);
}

#[test]
fn releases_listing_paginates_and_flattens_assets() {
    let server = TestServer::start(Arc::new(|path: &str, _hit| {
        if path.starts_with("/repos/owner/demo/releases?") && path.ends_with("page=1") {
            let page: Vec<Value> = (0..100)
                .map(|index| {
                    json!({
                        "tag_name": format!("v0.0.{}", index),
                        "name": "release",
                        "published_at": "2026-01-01T00:00:00Z",
                        "prerelease": index == 0,
                        "assets": [{ "name": "demo.zip", "size": 4,
                                     "browser_download_url": "http://127.0.0.1/x" }],
                    })
                })
                .collect();
            return Reply::ok(Value::Array(page).to_string());
        }
        if path.starts_with("/repos/owner/demo/releases?") && path.ends_with("page=2") {
            return Reply::ok(json!([{ "tag_name": "v1.0.0", "assets": [] }]).to_string());
        }
        Reply::status(404)
    }));
    let client = support::client(&server);
    let found = releases(&client, "owner/demo").unwrap();
    assert_eq!(found.len(), 101);
    assert!(found[0].prerelease);
    assert_eq!(found[0].assets.len(), 1);
    assert_eq!(
        found[0].published_at.as_deref(),
        Some("2026-01-01T00:00:00Z")
    );
    assert_eq!(found[100].tag, "v1.0.0");
    assert!(found[100].assets.is_empty());
}

#[test]
fn a_release_page_that_is_not_json_is_unreachable() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::ok("nope")));
    let client = support::client(&server);
    let problem = github_release(&client, "owner/demo", "1.2.3").unwrap_err();
    assert!(!problem.is_not_found(), "{problem}");
}

/// The regression that made every options read fail on a mod whose release
/// publishes sha256sums.txt: pin resolution stops at the published digest, so a
/// caller that wants the archive has to get a real download.
#[test]
fn fetch_asset_downloads_even_when_sums_publish_the_digest() {
    let server = TestServer::start_based(|path, _hit, base| match path {
        p if p == TAG_V => Reply::ok(
            serde_json::to_vec(&release(
                base,
                "v1.2.3",
                &["demo-1.2.3.zip", "sha256sums.txt"],
            ))
            .unwrap(),
        ),
        "/dl/sha256sums.txt" => Reply::ok(format!("{} *demo-1.2.3.zip\n", HELLO_SHA)),
        "/dl/demo-1.2.3.zip" => Reply::ok("hello world"),
        _ => Reply::status(404),
    });
    let dir = tempdir::TempDir::new("fetch").unwrap();
    let dest = dir.path().join("demo.zip");
    let client = support::client(&server);
    let opts = resolve::http::DownloadOpts {
        dest: Some(&dest),
        max_bytes: None,
        cancel: None,
        progress: None,
        authed: false,
    };
    let got = resolve::github::fetch_asset(&client, "owner/demo", "1.2.3", "demo", &opts).unwrap();
    assert!(dest.is_file(), "the archive must be on disk");
    assert_eq!(std::fs::read(&dest).unwrap(), b"hello world");
    assert_eq!(got.sha256, HELLO_SHA);
    assert_eq!(got.published_sha256.as_deref(), Some(HELLO_SHA));
    assert_eq!(got.asset, "demo-1.2.3.zip");
}
