//! Vocabulary and limits carried over from cartkit.py and CartManifest.lua.
//! The drift guard in CI compares these against gen1recomp@dev.

use regex::Regex;
use std::sync::OnceLock;

pub const CARTKIT_VERSION: &str = "1.0.0";
pub const USER_AGENT: &str = concat!(
    "g1r-cart-maker/",
    env!("CARGO_PKG_VERSION"),
    " (gen1recomp custom carts; +https://github.com/bryanthaboi/g1r-cart-maker)"
);

pub const CART_FILE: &str = "cart.json";
pub const CART_EXT: &str = ".g1rcart";
pub const BUNDLE_FORMAT: &str = "g1rcart";
pub const BUNDLE_VERSION: u64 = 1;
pub const CART_SCHEMA: u64 = 1;

pub const BASES: [&str; 6] = ["red", "blue", "yellow", "gold", "silver", "crystal"];
pub const SEALS: [&str; 3] = ["sealed", "sealed+", "open"];
pub const FINISHES: [&str; 3] = ["sparkle", "holo", "sparkle+holo"];
/// GameSpeed.LEVELS (src/core/GameSpeed.lua).
pub const SPEED_LEVELS: [i64; 11] = [1, 2, 3, 4, 10, 20, 30, 50, 75, 100, 200];
/// cartkit refuses `local`; the runtime accepts it for an unpublished install.
pub const SOURCES: [&str; 2] = ["github", "gamebanana"];
pub const RUNTIME_SOURCES: [&str; 3] = ["github", "gamebanana", "local"];
pub const MOD_PROFILES: [&str; 3] = ["content", "overhaul", "total_conversion"];
pub const MOD_PERMISSIONS: [&str; 6] = [
    "network",
    "filesystem",
    "engine_internals",
    "steps",
    "background",
    "compute",
];

pub const MAX_MODS: usize = 64;
pub const MAX_OPTIONS: usize = 64;
pub const MAX_OPTION_KEY: usize = 64;
pub const MAX_OPTION_TEXT: usize = 256;
pub const MAX_ID: usize = 64;
pub const MAX_TITLE: usize = 48;
pub const MAX_AUTHOR: usize = 64;
pub const MAX_SUMMARY: usize = 120;
pub const MAX_LABEL_PATH: usize = 128;
pub const LABEL_WARN_BYTES: u64 = 256 * 1024;
pub const LABEL_MAX_BYTES: u64 = 1024 * 1024;
pub const PNG_SIGNATURE: [u8; 8] = [137, b'P', b'N', b'G', b'\r', b'\n', 26, 10];

pub const CART_KEYS: [&str; 16] = [
    "schema",
    "id",
    "title",
    "version",
    "author",
    "repo",
    "summary",
    "shell",
    "finish",
    "label",
    "base",
    "engine",
    "seal",
    "speeds",
    "mods",
    "load_order",
];
pub const MOD_KEYS: [&str; 10] = [
    "id", "source", "repo", "version", "sha256", "mod", "file", "md5", "enabled", "options",
];

pub const PLACEHOLDER_REPO: &str = "owner/example-mod";
pub const PLACEHOLDER_SHA: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
pub const DEFAULT_SHELL: &str = "#8b1a1a";

macro_rules! lazy_regex {
    ($name:ident, $pattern:expr) => {
        pub fn $name() -> &'static Regex {
            static CELL: OnceLock<Regex> = OnceLock::new();
            CELL.get_or_init(|| Regex::new($pattern).expect("static pattern"))
        }
    };
}

lazy_regex!(id_re, r"^[A-Za-z0-9_-]{1,64}$");
lazy_regex!(
    semver_re,
    r"^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
);
lazy_regex!(shell_re, r"^#[0-9a-fA-F]{6}$");
lazy_regex!(sha256_re, r"^[0-9a-f]{64}$");
lazy_regex!(md5_re, r"^[0-9a-f]{32}$");
lazy_regex!(repo_re, r"^[A-Za-z0-9][\w.\-]*/[A-Za-z0-9][\w.\-]*$");
lazy_regex!(label_re, r"^[A-Za-z0-9_][A-Za-z0-9_.\-]*$");
lazy_regex!(control_re, r"[\x00-\x1f\x7f]");

lazy_regex!(
    github_spec_re,
    r"^(?:https?://github\.com/)?([\w.\-]+)/([\w.\-]+?)(?:\.git)?@(.+)$"
);
lazy_regex!(
    github_slug_re,
    r"^(?:https?://github\.com/)?([\w.\-]+)/([\w.\-]+?)(?:\.git)?/?$"
);
lazy_regex!(
    gamebanana_spec_re,
    r"^(?:gamebanana:|(?:https?://)?(?:www\.)?gamebanana\.com/mods/)?(\d+)/?$"
);

pub fn is_base(value: &str) -> bool {
    BASES.contains(&value)
}

pub fn is_seal(value: &str) -> bool {
    SEALS.contains(&value)
}

pub fn is_finish(value: &str) -> bool {
    FINISHES.contains(&value)
}
