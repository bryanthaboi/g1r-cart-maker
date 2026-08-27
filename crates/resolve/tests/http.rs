mod support;

use resolve::http::{
    backoff_delay, transport_delay, DownloadOpts, HttpError, MAX_BACKOFF, RATE_LIMIT_MESSAGE,
};
use resolve::{cancel_flag, github_token};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use support::{Reply, TestServer};

const HELLO_SHA: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
const HELLO_MD5: &str = "5eb63bbbe01eeed093cb22bb8f5acdc3";

#[test]
fn backoff_follows_cartkit_ladder() {
    assert_eq!(backoff_delay(0, None), 2.0);
    assert_eq!(backoff_delay(1, None), 4.0);
    assert_eq!(backoff_delay(3, None), 16.0);
    assert_eq!(backoff_delay(4, None), MAX_BACKOFF);
    assert_eq!(backoff_delay(0, Some(5.0)), 5.0);
    assert_eq!(backoff_delay(0, Some(900.0)), MAX_BACKOFF);
    assert_eq!(backoff_delay(0, Some(f64::NAN)), 2.0);
    assert_eq!(transport_delay(0), 1.0);
    assert_eq!(transport_delay(3), 8.0);
    assert_eq!(transport_delay(9), MAX_BACKOFF);
}

#[test]
fn quote_escapes_build_metadata() {
    assert_eq!(resolve::http::quote("v1.2.3"), "v1.2.3");
    assert_eq!(resolve::http::quote("1.0.0+build"), "1.0.0%2Bbuild");
    assert_eq!(resolve::http::quote("a b/c"), "a%20b/c");
}

#[test]
fn token_discovery_order() {
    let names = ["CARTKIT_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"];
    let saved: Vec<Option<String>> = names.iter().map(|name| std::env::var(name).ok()).collect();
    for name in names {
        std::env::remove_var(name);
    }

    assert_eq!(github_token(None), None);
    std::env::set_var("GH_TOKEN", "third");
    assert_eq!(github_token(None).as_deref(), Some("third"));
    std::env::set_var("GITHUB_TOKEN", "second");
    assert_eq!(github_token(None).as_deref(), Some("second"));
    std::env::set_var("CARTKIT_GITHUB_TOKEN", "  first  ");
    assert_eq!(github_token(None).as_deref(), Some("first"));
    assert_eq!(github_token(Some("explicit")).as_deref(), Some("explicit"));
    assert_eq!(github_token(Some("   ")).as_deref(), Some("first"));

    for (name, value) in names.iter().zip(saved) {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

#[test]
fn missing_is_not_found() {
    let server = TestServer::fixtures(vec![]);
    let client = support::client(&server);
    let problem = client
        .get_text(&server.url("/nope"), None, false)
        .unwrap_err();
    assert!(problem.is_not_found(), "{problem}");
}

#[test]
fn gone_is_not_found() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::status(410)));
    let client = support::client(&server);
    let problem = client
        .get_text(&server.url("/gone"), None, false)
        .unwrap_err();
    assert!(problem.is_not_found(), "{problem}");
}

#[test]
fn rate_limit_is_unreachable_with_cartkit_message() {
    let server = TestServer::start(Arc::new(|_path, _hit| {
        Reply::status(403).header("X-RateLimit-Remaining", "0")
    }));
    let client = support::client(&server);
    let problem = client
        .get_text(&server.url("/limited"), None, true)
        .unwrap_err();
    assert!(!problem.is_not_found());
    assert_eq!(problem.to_string(), RATE_LIMIT_MESSAGE);
    assert_eq!(server.hits("/limited"), 1, "a rate limit is not retried");
}

#[test]
fn server_error_retries_then_reports_status_and_reason() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::status(503)));
    let client = support::client(&server);
    let url = server.url("/down");
    let problem = client.get_text(&url, None, false).unwrap_err();
    assert_eq!(
        problem.to_string(),
        format!("{}: HTTP 503 Service Unavailable", url)
    );
    assert_eq!(server.hits("/down"), 3, "three attempts, as cartkit does");
}

#[test]
fn retry_then_success() {
    let server = TestServer::start(Arc::new(|_path, hit| {
        if hit < 3 {
            Reply::status(500)
        } else {
            Reply::ok("{\"ok\":true}")
        }
    }));
    let client = support::client(&server);
    let body = client.get_json(&server.url("/flaky"), None, false).unwrap();
    assert_eq!(body["ok"], serde_json::json!(true));
    assert_eq!(server.hits("/flaky"), 3);
}

#[test]
fn retry_after_is_honored_over_the_default_ladder() {
    let with_header = TestServer::start(Arc::new(|_path, hit| {
        if hit == 1 {
            Reply::status(429).header("Retry-After", "20")
        } else {
            Reply::ok("done")
        }
    }));
    let mut client = resolve::Client::new(None);
    client.set_wait_scale(0.02);
    let started = Instant::now();
    assert_eq!(
        client
            .get_text(&with_header.url("/slow"), None, false)
            .unwrap(),
        "done"
    );
    let waited = started.elapsed().as_secs_f64();
    assert!(
        waited >= 0.3,
        "Retry-After 20 should dominate, waited {waited}"
    );

    let plain = TestServer::start(Arc::new(|_path, hit| {
        if hit == 1 {
            Reply::status(429)
        } else {
            Reply::ok("done")
        }
    }));
    let started = Instant::now();
    assert_eq!(
        client.get_text(&plain.url("/slow"), None, false).unwrap(),
        "done"
    );
    let waited = started.elapsed().as_secs_f64();
    assert!(waited < 0.3, "the default ladder is 2s, waited {waited}");
}

#[test]
fn not_json_is_unreachable() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::ok("<html>")));
    let client = support::client(&server);
    let problem = client
        .get_json(&server.url("/page"), None, false)
        .unwrap_err();
    assert!(
        problem.to_string().contains("response was not JSON"),
        "{problem}"
    );
}

#[test]
fn download_hashes_both_digests_and_reports_progress() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::ok("hello world")));
    let client = support::client(&server);
    let seen = std::sync::Mutex::new(Vec::new());
    let progress = |done: u64, total: Option<u64>| {
        seen.lock().unwrap().push((done, total));
    };
    let got = client
        .download(
            &server.url("/mod.zip"),
            &DownloadOpts {
                progress: Some(&progress),
                ..DownloadOpts::default()
            },
        )
        .unwrap();
    assert_eq!(got.bytes, 11);
    assert_eq!(got.sha256, HELLO_SHA);
    assert_eq!(got.md5, HELLO_MD5);
    assert_eq!(got.path, None);
    assert_eq!(seen.lock().unwrap().as_slice(), &[(11, Some(11))]);
}

#[test]
fn download_writes_the_destination_and_verifies() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::ok("hello world")));
    let client = support::client(&server);
    let dir = tempdir::TempDir::new("download").unwrap();
    let dest = dir.path().join("mod-1.0.0.zip");
    let got = client
        .download(
            &server.url("/mod.zip"),
            &DownloadOpts {
                dest: Some(&dest),
                ..DownloadOpts::default()
            },
        )
        .unwrap();
    assert_eq!(got.path.as_deref(), Some(dest.as_path()));
    assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello world");
    assert!(!dir.path().join("mod-1.0.0.zip.part").exists());
}

#[test]
fn size_cap_refuses_and_leaves_nothing_behind() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::ok(vec![7u8; 400_000])));
    let client = support::client(&server);
    let dir = tempdir::TempDir::new("cap").unwrap();
    let dest = dir.path().join("big.zip");
    let problem = client
        .download(
            &server.url("/big.zip"),
            &DownloadOpts {
                dest: Some(&dest),
                max_bytes: Some(1024),
                ..DownloadOpts::default()
            },
        )
        .unwrap_err();
    assert!(
        matches!(problem, HttpError::TooLarge { limit: 1024, .. }),
        "{problem}"
    );
    assert!(!dest.exists());
    assert!(!dir.path().join("big.zip.part").exists());
}

#[test]
fn cancel_mid_download_leaves_no_partial_file() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::ok(vec![3u8; 2_000_000])));
    let client = support::client(&server);
    let dir = tempdir::TempDir::new("cancel").unwrap();
    let dest = dir.path().join("mod.zip");
    let cancel = cancel_flag();
    let flag = Arc::clone(&cancel);
    let progress = move |_done: u64, _total: Option<u64>| {
        flag.store(true, Ordering::Relaxed);
    };
    let problem = client
        .download(
            &server.url("/mod.zip"),
            &DownloadOpts {
                dest: Some(&dest),
                cancel: Some(&cancel),
                progress: Some(&progress),
                ..DownloadOpts::default()
            },
        )
        .unwrap_err();
    assert!(matches!(problem, HttpError::Cancelled), "{problem}");
    assert!(!dest.exists());
    assert!(!dir.path().join("mod.zip.part").exists());
}

#[test]
fn a_cancelled_flag_stops_before_the_first_chunk() {
    let server = TestServer::start(Arc::new(|_path, _hit| Reply::ok("hello world")));
    let client = support::client(&server);
    let cancel = cancel_flag();
    cancel.store(true, Ordering::Relaxed);
    let problem = client
        .download(
            &server.url("/mod.zip"),
            &DownloadOpts {
                cancel: Some(&cancel),
                ..DownloadOpts::default()
            },
        )
        .unwrap_err();
    assert!(matches!(problem, HttpError::Cancelled), "{problem}");
}
