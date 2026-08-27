//! The live index-readiness checklist: what a cart repo needs before
//! `ModIndex.parseCartEntry` will list it. Remote facts come from gh.

use crate::detect;
use crate::runner::{CancelToken, Invocation, RunError, Runner};
use cartcore::cart::{cart_str, mods_of, Cart};
use cartcore::pack::bundle_name;
use cartcore::schema::{md5_re, sha256_re, MAX_SUMMARY};
use serde::Serialize;
use serde_json::Value;

pub const SHA_SUMS: &str = "sha256sums.txt";

/// The recommended index fields. cartkit warns on any extra cart.json key, so
/// these live in the submission draft and only fall back to cart.json.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IndexHints {
    pub thumbnail: Option<String>,
    pub description_url: Option<String>,
    pub license: Option<String>,
    pub tags: Vec<String>,
    pub automatic_version_check: Option<bool>,
    pub fixed_release_tag: Option<String>,
}

impl IndexHints {
    /// A cart that carries the fields anyway (at the cost of a CK001 warning).
    pub fn from_cart(cart: &Cart) -> Self {
        let text = |key: &str| {
            cart_str(cart, key)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        };
        Self {
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
    }
}

/// A fix the UI can offer as one click. `command` is an argument array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fix {
    pub id: String,
    pub label: String,
    pub command: Option<Vec<String>>,
}

impl Fix {
    fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            command: None,
        }
    }

    fn with_command(id: &str, label: &str, command: Vec<String>) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            command: Some(command),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadinessItem {
    pub id: String,
    pub label: String,
    pub ok: bool,
    /// A blocking item unmet keeps the cart out of the index.
    pub blocking: bool,
    pub detail: String,
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Readiness {
    pub items: Vec<ReadinessItem>,
    pub ready: bool,
    /// Facts gh could not supply, so the UI can say "unknown" rather than "no".
    pub unknown: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReleaseFacts {
    pub tag: String,
    pub url: Option<String>,
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RemoteFacts {
    pub slug: String,
    pub visibility: Option<String>,
    pub is_private: Option<bool>,
    pub url: Option<String>,
    pub license: Option<String>,
    pub release: Option<ReleaseFacts>,
    /// Anything gh refused to answer, verbatim.
    pub problems: Vec<String>,
}

pub fn fetch_remote(
    runner: &dyn Runner,
    cancel: &CancelToken,
    slug: &str,
    tag: &str,
) -> Result<RemoteFacts, RunError> {
    let mut facts = RemoteFacts {
        slug: slug.to_string(),
        ..RemoteFacts::default()
    };

    let repo = runner.run(
        &Invocation::new(
            detect::GH,
            [
                "repo",
                "view",
                slug,
                "--json",
                "visibility,isPrivate,url,licenseInfo",
            ],
        ),
        cancel,
    )?;
    if repo.success() {
        let doc: Value = serde_json::from_str(&repo.stdout).unwrap_or(Value::Null);
        facts.visibility = doc
            .get("visibility")
            .and_then(Value::as_str)
            .map(|value| value.to_ascii_lowercase());
        facts.is_private = doc.get("isPrivate").and_then(Value::as_bool);
        facts.url = doc.get("url").and_then(Value::as_str).map(str::to_string);
        facts.license = doc
            .get("licenseInfo")
            .and_then(|info| info.get("spdxId").or_else(|| info.get("name")))
            .and_then(Value::as_str)
            .map(str::to_string);
    } else {
        facts.problems.push(repo.problem());
    }

    let release = runner.run(
        &Invocation::new(
            detect::GH,
            [
                "release",
                "view",
                tag,
                "--repo",
                slug,
                "--json",
                "tagName,url,assets",
            ],
        ),
        cancel,
    )?;
    if release.success() {
        let doc: Value = serde_json::from_str(&release.stdout).unwrap_or(Value::Null);
        facts.release = Some(ReleaseFacts {
            tag: doc
                .get("tagName")
                .and_then(Value::as_str)
                .unwrap_or(tag)
                .to_string(),
            url: doc.get("url").and_then(Value::as_str).map(str::to_string),
            assets: crate::publish::asset_names(&doc),
        });
    } else {
        facts.problems.push(release.problem());
    }
    Ok(facts)
}

/// Fetch and evaluate in one call, which is what the UI panel does.
pub fn check(
    runner: &dyn Runner,
    cancel: &CancelToken,
    cart: &Cart,
    slug: &str,
    hints: &IndexHints,
) -> Result<Readiness, RunError> {
    let tag = format!("v{}", cart_str(cart, "version").unwrap_or_default());
    let facts = fetch_remote(runner, cancel, slug, &tag)?;
    Ok(evaluate(cart, Some(&facts), hints))
}

const REQUIRED: [&str; 8] = [
    "id", "title", "author", "version", "base", "seal", "repo", "mods",
];

pub fn evaluate(cart: &Cart, remote: Option<&RemoteFacts>, hints: &IndexHints) -> Readiness {
    let mut items = Vec::new();
    let mut unknown = Vec::new();
    let version = cart_str(cart, "version").unwrap_or_default().to_string();
    let tag = format!("v{}", version);
    let asset = bundle_name(cart);
    let slug = remote
        .map(|facts| facts.slug.clone())
        .unwrap_or_else(|| cart_str(cart, "repo").unwrap_or_default().to_string());

    // repo is public
    match remote.and_then(|facts| {
        facts
            .is_private
            .map(|private| !private)
            .or_else(|| facts.visibility.as_ref().map(|value| value == "public"))
    }) {
        Some(public) => items.push(ReadinessItem {
            id: "repo_public".into(),
            label: "The repository is public".into(),
            ok: public,
            blocking: true,
            detail: if public {
                format!("{} is public", slug)
            } else {
                format!("{} is private; a private repo cannot be indexed", slug)
            },
            fix: (!public).then(|| {
                Fix::with_command(
                    "make_public",
                    "Make the repository public",
                    vec![
                        "gh".into(),
                        "repo".into(),
                        "edit".into(),
                        slug.clone(),
                        "--visibility".into(),
                        "public".into(),
                    ],
                )
            }),
        }),
        None => {
            unknown.push("repo_public".to_string());
            items.push(ReadinessItem {
                id: "repo_public".into(),
                label: "The repository is public".into(),
                ok: false,
                blocking: true,
                detail: "the repository could not be read; sign in and re-check".into(),
                fix: Some(Fix::new("recheck", "Re-check")),
            });
        }
    }

    // schema 1 at the repo root
    let schema = cart.get("schema").and_then(Value::as_i64);
    items.push(ReadinessItem {
        id: "cart_schema".into(),
        label: "cart.json at the repo root, schema 1".into(),
        ok: schema == Some(1),
        blocking: true,
        detail: match schema {
            Some(1) => "schema 1".to_string(),
            Some(other) => format!("schema {} is not the version the index reads", other),
            None => "cart.json has no schema field".to_string(),
        },
        fix: (schema != Some(1)).then(|| Fix::new("set_schema", "Set schema to 1")),
    });

    // the eight required fields
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|key| !present(cart, key))
        .collect();
    items.push(ReadinessItem {
        id: "required_fields".into(),
        label: "The eight required fields are present".into(),
        ok: missing.is_empty(),
        blocking: true,
        detail: if missing.is_empty() {
            "id, title, author, version, base, seal, repo and mods are all set".to_string()
        } else {
            format!("missing: {}", missing.join(", "))
        },
        fix: (!missing.is_empty()).then(|| Fix::new("edit_cart", "Fill the missing fields")),
    });

    // at least one valid pin
    let pins = mods_of(cart)
        .iter()
        .filter(|entry| valid_pin(entry))
        .count();
    items.push(ReadinessItem {
        id: "valid_pin".into(),
        label: "At least one valid pin".into(),
        ok: pins > 0,
        blocking: true,
        detail: if pins > 0 {
            format!("{} pinned mod(s) resolve", pins)
        } else {
            "no mod entry carries a complete pin".to_string()
        },
        fix: (pins == 0).then(|| Fix::new("add_mod", "Add a mod")),
    });

    // the release
    let release = remote.and_then(|facts| facts.release.as_ref());
    match release {
        Some(found) => items.push(ReadinessItem {
            id: "release_tag".into(),
            label: format!("A release tagged {}", tag),
            ok: found.tag == tag,
            blocking: true,
            detail: if found.tag == tag {
                format!("{} matches cart.json", tag)
            } else {
                format!(
                    "the release is tagged {}, cart.json says {}",
                    found.tag, tag
                )
            },
            fix: (found.tag != tag).then(|| Fix::new("publish_release", "Tag and publish")),
        }),
        None => items.push(ReadinessItem {
            id: "release_tag".into(),
            label: format!("A release tagged {}", tag),
            ok: false,
            blocking: true,
            detail: format!("no release tagged {} was found", tag),
            fix: Some(Fix::new("publish_release", "Tag and publish")),
        }),
    }

    let assets = release
        .map(|found| found.assets.clone())
        .unwrap_or_default();
    let attached = assets.iter().any(|name| name == &asset);
    items.push(ReadinessItem {
        id: "bundle_asset".into(),
        label: format!("{} attached to the release", asset),
        ok: attached,
        blocking: true,
        detail: if attached {
            format!("{} is attached", asset)
        } else if assets.is_empty() {
            format!("{} is not attached; the release has no assets", asset)
        } else {
            format!("attached instead: {}", assets.join(", "))
        },
        fix: (!attached).then(|| Fix::new("rerun_release", "Re-run the release workflow")),
    });

    // recommended
    let has_sums = assets.iter().any(|name| name == SHA_SUMS);
    items.push(recommended(
        "sha256sums",
        format!("{} published with the release", SHA_SUMS),
        has_sums,
        if has_sums {
            format!("{} is attached", SHA_SUMS)
        } else {
            format!("{} lets the launcher verify the download", SHA_SUMS)
        },
        (!has_sums).then(|| Fix::new("rerun_release", "Re-run the release workflow")),
    ));

    for (id, label, value) in [
        ("thumbnail", "A thumbnail", hints.thumbnail.clone()),
        (
            "description_url",
            "A description URL",
            hints.description_url.clone(),
        ),
    ] {
        let value = value.unwrap_or_default();
        items.push(recommended(
            id,
            label.to_string(),
            !value.is_empty(),
            if value.is_empty() {
                format!("{} is not set for the index entry", id)
            } else {
                value.clone()
            },
            value
                .is_empty()
                .then(|| Fix::new("edit_entry", &format!("Set {}", id))),
        ));
    }

    let license = hints
        .license
        .clone()
        .or_else(|| remote.and_then(|facts| facts.license.clone()))
        .unwrap_or_default();
    items.push(recommended(
        "license",
        "A license".to_string(),
        !license.is_empty(),
        if license.is_empty() {
            "no license on the cart or the repository".to_string()
        } else {
            license.clone()
        },
        license
            .is_empty()
            .then(|| Fix::new("edit_entry", "Add a license")),
    ));

    let summary = cart_str(cart, "summary").unwrap_or_default();
    let summary_ok = !summary.is_empty() && summary.chars().count() <= MAX_SUMMARY;
    items.push(recommended(
        "summary",
        format!("A summary of {} characters or fewer", MAX_SUMMARY),
        summary_ok,
        if summary.is_empty() {
            "no summary; the index row will read as a bare title".to_string()
        } else if summary.chars().count() > MAX_SUMMARY {
            format!(
                "{} characters, {} allowed",
                summary.chars().count(),
                MAX_SUMMARY
            )
        } else {
            summary.to_string()
        },
        (!summary_ok).then(|| Fix::new("edit_cart", "Edit the summary")),
    ));

    let tags = hints.tags.len();
    items.push(recommended(
        "tags",
        "Tags".to_string(),
        tags > 0,
        if tags > 0 {
            format!("{} tag(s)", tags)
        } else {
            "no tags; the cart will only be found by name".to_string()
        },
        (tags == 0).then(|| Fix::new("edit_entry", "Add tags")),
    ));

    let automatic = hints.automatic_version_check.unwrap_or(true);
    let fixed = hints.fixed_release_tag.clone().unwrap_or_default();
    let version_ok = automatic || !fixed.is_empty();
    items.push(recommended(
        "version_check",
        "Version checking".to_string(),
        version_ok,
        if automatic {
            "automatic_version_check is on; the index follows the newest release".to_string()
        } else if !fixed.is_empty() {
            format!("pinned to {}", fixed)
        } else {
            "automatic_version_check is off with no fixed_release_tag; the index cannot resolve a download"
                .to_string()
        },
        (!version_ok).then(|| Fix::new("edit_entry", "Turn automatic_version_check back on")),
    ));

    let ready = items.iter().all(|item| item.ok || !item.blocking);
    Readiness {
        items,
        ready,
        unknown,
    }
}

fn recommended(
    id: &str,
    label: String,
    ok: bool,
    detail: String,
    fix: Option<Fix>,
) -> ReadinessItem {
    ReadinessItem {
        id: id.to_string(),
        label,
        ok,
        blocking: false,
        detail,
        fix,
    }
}

fn present(cart: &Cart, key: &str) -> bool {
    match cart.get(key) {
        Some(Value::String(text)) => !text.trim().is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Null) | None => false,
        Some(_) => true,
    }
}

/// A pin the index would accept: an id plus the digest its source needs.
pub fn valid_pin(entry: &Value) -> bool {
    let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
    if id.is_empty() {
        return false;
    }
    let source = entry
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("github");
    let text = |key: &str| entry.get(key).and_then(Value::as_str).unwrap_or_default();
    match source {
        "gamebanana" => {
            !text("mod").is_empty() && !text("file").is_empty() && md5_re().is_match(text("md5"))
        }
        _ => {
            !text("repo").is_empty()
                && !text("version").is_empty()
                && sha256_re().is_match(text("sha256"))
        }
    }
}
