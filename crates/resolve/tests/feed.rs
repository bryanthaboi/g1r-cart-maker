mod support;

use cartcore::index::{IndexSource, CACHE_TTL};
use resolve::feed::{now_seconds, Cached, FeedCache, FeedError};
use serde_json::json;
use std::sync::Arc;
use support::{Reply, TestServer};

const FEED: &str = "/data/index.json";
const RAW: &str = "/raw/index.json";

fn body(title: &str) -> String {
    json!({
        "schema_version": 1,
        "generated_at": "2026-01-01T00:00:00Z",
        "mods": [{ "id": "demo", "title": title, "summary": "a mod" }],
    })
    .to_string()
}

fn source(server: &TestServer, fallback: bool) -> IndexSource {
    IndexSource {
        feed: server.url(FEED),
        base: format!("{}/", server.base),
        fallback: fallback.then(|| server.url(RAW)),
        label: "loopback/feed".to_string(),
    }
}

fn cache() -> (tempdir::TempDir, FeedCache) {
    let dir = tempdir::TempDir::new("feed").unwrap();
    let cache = FeedCache::new(dir.path());
    (dir, cache)
}

#[test]
fn a_fresh_fetch_is_cached_and_then_served_from_disk() {
    let server = TestServer::start(Arc::new(|path: &str, _hit| match path {
        FEED => Reply::ok(body("Demo")),
        _ => Reply::status(404),
    }));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let source = source(&server, false);

    let first = cache
        .load_source(&client, source.clone(), false, false)
        .unwrap();
    assert!(!first.from_cache);
    assert!(!first.stale);
    assert!(!first.from_fallback);
    assert_eq!(first.index.mods[0].title, "Demo");

    let second = cache
        .load_source(&client, source.clone(), false, false)
        .unwrap();
    assert!(second.from_cache);
    assert!(!second.stale);
    assert_eq!(server.hits(FEED), 1, "a fresh cache does not go out");
}

#[test]
fn a_manual_refresh_ignores_a_fresh_cache() {
    let server = TestServer::start(Arc::new(|path: &str, hit| match path {
        FEED if hit == 1 => Reply::ok(body("Old")),
        FEED => Reply::ok(body("New")),
        _ => Reply::status(404),
    }));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let source = source(&server, false);

    assert_eq!(
        cache
            .load_source(&client, source.clone(), false, false)
            .unwrap()
            .index
            .mods[0]
            .title,
        "Old"
    );
    let refreshed = cache
        .load_source(&client, source.clone(), true, false)
        .unwrap();
    assert_eq!(refreshed.index.mods[0].title, "New");
    assert!(!refreshed.from_cache);
    assert_eq!(server.hits(FEED), 2);
}

#[test]
fn a_cache_past_the_ttl_is_refetched() {
    let server = TestServer::start(Arc::new(|path: &str, _hit| match path {
        FEED => Reply::ok(body("New")),
        _ => Reply::status(404),
    }));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let source = source(&server, false);
    cache
        .write_cached(
            &source,
            &Cached {
                feed: source.feed.clone(),
                fetched_at: now_seconds() - CACHE_TTL - 1,
                from_fallback: false,
                body: body("Old"),
            },
        )
        .unwrap();

    let got = cache
        .load_source(&client, source.clone(), false, false)
        .unwrap();
    assert_eq!(got.index.mods[0].title, "New");
    assert!(!got.from_cache);
    assert_eq!(server.hits(FEED), 1);
}

#[test]
fn the_raw_url_is_only_tried_when_the_feed_fails() {
    let server = TestServer::start(Arc::new(|path: &str, _hit| match path {
        FEED => Reply::ok(body("Pages")),
        RAW => Reply::ok(body("Raw")),
        _ => Reply::status(404),
    }));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let got = cache
        .load_source(&client, source(&server, true), false, false)
        .unwrap();
    assert_eq!(got.index.mods[0].title, "Pages");
    assert!(!got.from_fallback);
    assert_eq!(server.hits(RAW), 0);
}

#[test]
fn the_raw_url_rescues_a_dead_feed() {
    let server = TestServer::start(Arc::new(|path: &str, _hit| match path {
        RAW => Reply::ok(body("Raw")),
        _ => Reply::status(404),
    }));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let source = source(&server, true);
    let got = cache
        .load_source(&client, source.clone(), false, false)
        .unwrap();
    assert_eq!(got.index.mods[0].title, "Raw");
    assert!(got.from_fallback);
    assert!(!got.from_cache);
    assert!(cache.read_cached(&source).unwrap().from_fallback);
}

#[test]
fn a_dead_network_serves_the_stale_cache() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::status(503)));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let source = source(&server, false);
    cache
        .write_cached(
            &source,
            &Cached {
                feed: source.feed.clone(),
                fetched_at: now_seconds() - CACHE_TTL - 1,
                from_fallback: false,
                body: body("Old"),
            },
        )
        .unwrap();

    let got = cache
        .load_source(&client, source.clone(), false, false)
        .unwrap();
    assert_eq!(got.index.mods[0].title, "Old");
    assert!(got.from_cache);
    assert!(got.stale);
}

#[test]
fn a_dead_network_with_no_cache_is_an_error() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::status(503)));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let problem = cache
        .load_source(&client, source(&server, false), false, false)
        .unwrap_err();
    assert!(matches!(problem, FeedError::Fetch(_)), "{problem}");
}

#[test]
fn offline_serves_the_cache_and_never_calls_out() {
    let server = TestServer::start(Arc::new(|path: &str, _hit| match path {
        FEED => Reply::ok(body("Network")),
        _ => Reply::status(404),
    }));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let source = source(&server, false);
    cache
        .write_cached(
            &source,
            &Cached {
                feed: source.feed.clone(),
                fetched_at: now_seconds() - CACHE_TTL - 1,
                from_fallback: false,
                body: body("Cached"),
            },
        )
        .unwrap();

    let got = cache
        .load_source(&client, source.clone(), true, true)
        .unwrap();
    assert_eq!(got.index.mods[0].title, "Cached");
    assert!(got.from_cache);
    assert!(got.stale);
    assert_eq!(server.hits(FEED), 0);
}

#[test]
fn offline_with_no_cache_says_so() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let problem = cache
        .load_source(&client, source(&server, false), false, true)
        .unwrap_err();
    assert!(matches!(problem, FeedError::NotCached(_)), "{problem}");
}

#[test]
fn an_unparsable_feed_is_a_parse_error_and_is_not_cached() {
    let server = TestServer::start(Arc::new(|path: &str, _hit| match path {
        FEED => Reply::ok("{\"schema_version\": 9, \"mods\": []}"),
        _ => Reply::status(404),
    }));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let source = source(&server, false);
    let problem = cache
        .load_source(&client, source.clone(), false, false)
        .unwrap_err();
    assert!(matches!(problem, FeedError::Parse(_)), "{problem}");
    assert!(cache.read_cached(&source).is_none());
}

#[test]
fn a_bad_index_url_is_a_source_error() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let problem = cache.load(&client, "   ", false, false).unwrap_err();
    assert!(matches!(problem, FeedError::Source(_)), "{problem}");
}

#[test]
fn thumbnails_cache_by_url_and_keep_their_type() {
    let server = TestServer::start(Arc::new(|path: &str, _hit| match path {
        "/thumb.png" => Reply::ok(vec![137, 80, 78, 71]).header("Content-Type", "image/png"),
        _ => Reply::status(404),
    }));
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let url = server.url("/thumb.png");

    let first = cache.thumbnail(&client, &url).unwrap();
    assert_eq!(first.bytes, vec![137, 80, 78, 71]);
    assert_eq!(first.content_type, "image/png");
    assert!(!first.from_cache);

    let second = cache.thumbnail(&client, &url).unwrap();
    assert!(second.from_cache);
    assert_eq!(second.bytes, first.bytes);
    assert_eq!(second.content_type, "image/png");
    assert_eq!(server.hits("/thumb.png"), 1);
}

#[test]
fn one_failing_thumbnail_is_just_an_error() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    let (_dir, cache) = cache();
    let problem = cache
        .thumbnail(&client, &server.url("/missing.png"))
        .unwrap_err();
    assert!(matches!(problem, FeedError::Fetch(_)), "{problem}");
    assert!(cache
        .thumbnail(&client, &server.url("/missing.png"))
        .is_err());
}
