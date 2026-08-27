//! Everything in the cart tooling that touches the network or an archive:
//! release resolution, GameBanana, online validation, index feeds and zips.
//! The offline half lives in `cartcore` and is never duplicated here.

pub mod archive;
pub mod feed;
pub mod gamebanana;
pub mod github;
pub mod http;
pub mod online;

pub use archive::{extract_zip, read_entry, verify, ArchiveCache, ArchiveError, Limits};
pub use feed::{FeedCache, FeedError, Fetched, Thumbnail};
pub use gamebanana::{gamebanana_file, gamebanana_files, pin_gamebanana, GbFile, GbPin};
pub use github::{pin_github, releases, resolve_github, Asset, ReleaseSummary, Resolved};
pub use http::{cancel_flag, github_token, CancelFlag, Client, Download, DownloadOpts, HttpError};
pub use online::online_findings;

/// Offline schema checks plus the online pass, as one report.
pub fn validate_online(
    cart: &cartcore::Cart,
    cart_dir: Option<&std::path::Path>,
    download: bool,
    token: Option<&str>,
    cancel: &CancelFlag,
) -> cartcore::Report {
    let mut findings = cartcore::schema_findings(cart, cart_dir);
    let (online, notes) = online_findings(cart, download, token, cancel);
    findings.extend(online);
    cartcore::Report { findings, notes }
}
