//! Offline schema rules, ported rule for rule and message for message from
//! cartkit.py so a finding here reads identically to one from CI.

use crate::cart::Cart;
use crate::findings::{err, err_at, warn, warn_at, Finding};
use crate::schema::*;
use serde_json::Value;
use std::fs;
use std::path::Path;

fn chars(text: &str) -> usize {
    text.chars().count()
}

fn full_match(pattern: &regex::Regex, value: Option<&Value>) -> bool {
    matches!(value, Some(Value::String(text)) if pattern.is_match(text))
}

pub fn text_problem(value: Option<&Value>, low: usize, high: usize) -> Option<String> {
    let text = match value {
        Some(Value::String(text)) => text,
        _ => return Some("must be a string".into()),
    };
    if control_re().is_match(text) {
        return Some("must not contain control characters".into());
    }
    let length = chars(text);
    if length < low {
        return Some(format!("must be at least {} character(s)", low));
    }
    if length > high {
        return Some(format!(
            "must be at most {} characters (got {})",
            high, length
        ));
    }
    None
}

const RANGE_OPS: [&str; 8] = ["", "=", "==", ">", ">=", "<", "<=", "^"];

fn range_head(token: &str) -> &str {
    let end = token
        .find(|c| !matches!(c, '=' | '<' | '>' | '^'))
        .unwrap_or(token.len());
    &token[..end]
}

fn range_core_ok(text: &str) -> bool {
    static CELL: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = CELL.get_or_init(|| {
        regex::Regex::new(r"^[vV]?\d+(?:\.\d+){0,2}(?:-[0-9A-Za-z.\-]+)?(?:\+[0-9A-Za-z.\-]+)?$")
            .expect("static pattern")
    });
    pattern.is_match(text)
}

/// A semver range as cartkit accepts it: space-joined comparators, `||` alternatives.
pub fn range_problem(text: Option<&Value>) -> Option<String> {
    let text = match text {
        Some(Value::String(text)) if !text.trim().is_empty() => text,
        _ => return Some("must be a non-empty semver range string".into()),
    };
    for alternative in text.split("||") {
        let tokens: Vec<&str> = alternative.split_whitespace().collect();
        if tokens.is_empty() {
            return Some("has an empty alternative around '||'".into());
        }
        for token in tokens {
            let head = range_head(token);
            if !RANGE_OPS.contains(&head) {
                return Some(format!("has an unknown comparator {}", python_repr(head)));
            }
            if !range_core_ok(&token[head.len()..]) {
                return Some(format!(
                    "has an unparsable version in {}",
                    python_repr(token)
                ));
            }
        }
    }
    None
}

/// Python's `repr` for the strings cartkit interpolates into its messages.
pub fn python_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

pub fn label_problem(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(text) if !text.is_empty() => text,
        _ => return Some("must be a non-empty string".into()),
    };
    if chars(text) > MAX_LABEL_PATH {
        return Some(format!("must be at most {} characters", MAX_LABEL_PATH));
    }
    let drive =
        text.len() >= 2 && text.as_bytes()[0].is_ascii_alphabetic() && text.as_bytes()[1] == b':';
    if text.starts_with('/') || text.contains('\\') || drive {
        return Some("must be a relative path inside the cart".into());
    }
    let segments: Vec<&str> = text
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if segments.is_empty() || segments.contains(&"..") {
        return Some("must not leave the cart directory".into());
    }
    if !segments.iter().all(|s| label_re().is_match(s)) {
        return Some("must be a plain relative path (letters, digits, . _ - and /)".into());
    }
    None
}

pub fn option_problems(options: &Value, label: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let map = match options {
        Value::Object(map) => map,
        _ => return vec![format!("{} options must be an object", label)],
    };
    if map.len() > MAX_OPTIONS {
        problems.push(format!(
            "{} has {} options (max {})",
            label,
            map.len(),
            MAX_OPTIONS
        ));
    }
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    for key in keys {
        let value = &map[key];
        if control_re().is_match(key) {
            problems.push(format!(
                "{} option key {} must be plain text",
                label,
                python_repr(key)
            ));
            continue;
        }
        let key_length = chars(key);
        if !(1..=MAX_OPTION_KEY).contains(&key_length) {
            problems.push(format!(
                "{} option key {} must be 1..64 characters",
                label,
                python_repr(key)
            ));
        }
        match value {
            Value::Bool(_) => continue,
            Value::String(text) => {
                if control_re().is_match(text) {
                    problems.push(format!(
                        "{} option {} must not contain control characters",
                        label,
                        python_repr(key)
                    ));
                } else if chars(text) > MAX_OPTION_TEXT {
                    problems.push(format!(
                        "{} option {} is {} characters (max {})",
                        label,
                        python_repr(key),
                        chars(text),
                        MAX_OPTION_TEXT
                    ));
                }
            }
            Value::Number(number) => {
                if number.as_f64().map(|f| !f.is_finite()).unwrap_or(true) {
                    problems.push(format!(
                        "{} option {} must be a finite number",
                        label,
                        python_repr(key)
                    ));
                }
            }
            _ => problems.push(format!(
                "{} option {} must be a string, number or boolean",
                label,
                python_repr(key)
            )),
        }
    }
    problems
}

fn is_int(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Number(number)) if number.is_i64() || number.is_u64())
}

pub fn check_identity(cart: &Cart, findings: &mut Vec<Finding>) {
    match cart.get("schema").filter(|v| !v.is_null()) {
        None => findings.push(err("CK002", "\"schema\" is missing; add \"schema\": 1")),
        Some(Value::Number(number)) if number.is_i64() || number.is_u64() => {
            let schema = number.as_i64().unwrap_or_default();
            if schema != CART_SCHEMA as i64 {
                findings.push(err(
                    "CK002",
                    format!(
                        "\"schema\" is {}; this cartkit writes and reads schema {}",
                        schema, CART_SCHEMA
                    ),
                ));
            }
        }
        Some(_) => findings.push(err("CK002", "\"schema\" must be the number 1")),
    }

    if !full_match(id_re(), cart.get("id")) {
        findings.push(err(
            "CK002",
            "\"id\" must be 1..64 characters of letters, digits, _ or -",
        ));
    }

    for (key, low, high) in [("title", 1, MAX_TITLE), ("author", 1, MAX_AUTHOR)] {
        if let Some(problem) = text_problem(cart.get(key), low, high) {
            findings.push(err("CK002", format!("\"{}\" {}", key, problem)));
        }
    }

    if !full_match(semver_re(), cart.get("version")) {
        findings.push(err("CK002", "\"version\" must be semver, e.g. 1.0.0"));
    }

    if let Some(repo) = cart.get("repo").filter(|v| !v.is_null()) {
        if !repo_re().is_match(repo.as_str().unwrap_or("")) {
            findings.push(err("CK002", "\"repo\" must be owner/name"));
        }
    }

    if let Some(summary) = cart.get("summary").filter(|v| !v.is_null()) {
        if let Some(problem) = text_problem(Some(summary), 0, MAX_SUMMARY) {
            findings.push(err("CK002", format!("\"summary\" {}", problem)));
        }
    }

    if !full_match(shell_re(), cart.get("shell")) {
        findings.push(err(
            "CK002",
            "\"shell\" must be \"#rrggbb\", e.g. \"#8b1a1a\"",
        ));
    }

    if !cart
        .get("base")
        .and_then(Value::as_str)
        .map(is_base)
        .unwrap_or(false)
    {
        findings.push(err(
            "CK002",
            format!("\"base\" must be one of {}", BASES.join(", ")),
        ));
    }

    if let Some(engine) = cart.get("engine").filter(|v| !v.is_null()) {
        if let Some(problem) = range_problem(Some(engine)) {
            findings.push(err(
                "CK002",
                format!("\"engine\" {}; write it like \">=1.0.0 <2.0.0\"", problem),
            ));
        }
    }

    let seal = cart.get("seal").and_then(Value::as_str).unwrap_or("sealed");
    let seal_present = cart.get("seal").is_some();
    if (seal_present && !cart.get("seal").map(Value::is_string).unwrap_or(false)) || !is_seal(seal)
    {
        findings.push(err(
            "CK002",
            format!("\"seal\" must be one of: {}", SEALS.join(", ")),
        ));
    }

    if let Some(finish) = cart.get("finish").filter(|v| !v.is_null()) {
        if !finish.as_str().map(is_finish).unwrap_or(false) {
            findings.push(err(
                "CK002",
                format!("\"finish\" must be one of: {}", FINISHES.join(", ")),
            ));
        }
    }

    if let Some(speeds) = cart.get("speeds").filter(|v| !v.is_null()) {
        match speeds {
            Value::Array(items) if !items.is_empty() => {
                let known = items.iter().all(|item| {
                    item.as_i64()
                        .map(|n| SPEED_LEVELS.contains(&n))
                        .unwrap_or(false)
                });
                if !known {
                    findings.push(err(
                        "CK002",
                        format!(
                            "\"speeds\" entries must be game-speed levels: {}",
                            SPEED_LEVELS
                                .iter()
                                .map(|n| n.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                }
            }
            _ => findings.push(err("CK002", "\"speeds\" must be a non-empty array")),
        }
    }

    for key in cart.keys() {
        if !CART_KEYS.contains(&key.as_str()) {
            findings.push(warn(
                "CK001",
                format!(
                    "unknown field {}; cartkit packs only the documented fields",
                    python_repr(key)
                ),
            ));
        }
    }
}

pub fn check_label(cart: &Cart, cart_dir: Option<&Path>, findings: &mut Vec<Finding>) {
    let label = match cart.get("label").filter(|v| !v.is_null()) {
        Some(value) => value,
        None => return,
    };
    if let Some(problem) = label_problem(label) {
        findings.push(err("CK003", format!("\"label\" {}", problem)));
        return;
    }
    let label = label.as_str().unwrap_or_default();
    let cart_dir = match cart_dir {
        Some(dir) => dir,
        None => return,
    };
    let path = cart_dir.join(label);
    if !path.is_file() {
        findings.push(err_at(
            "CK003",
            format!(
                "label art {} is missing from the cart directory",
                python_repr(label)
            ),
            label,
        ));
        return;
    }
    let inside = match (path.canonicalize(), cart_dir.canonicalize()) {
        (Ok(art), Ok(root)) => art.starts_with(&root) && art != root,
        _ => true,
    };
    if !inside {
        findings.push(err_at(
            "CK003",
            format!(
                "label art {} resolves outside the cart directory",
                python_repr(label)
            ),
            label,
        ));
        return;
    }
    if !label.to_lowercase().ends_with(".png") {
        findings.push(warn_at(
            "CK003",
            "label art should be a .png; the game draws nothing else",
            label,
        ));
    }
    let size = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    if size > LABEL_MAX_BYTES {
        findings.push(err_at(
            "CK003",
            format!(
                "label art is {} bytes; keep it under {} so the bundle stays small",
                size, LABEL_MAX_BYTES
            ),
            label,
        ));
    } else if size > LABEL_WARN_BYTES {
        findings.push(warn_at(
            "CK003",
            format!(
                "label art is {} bytes; a cart label wants a few KB, not a photo",
                size
            ),
            label,
        ));
    }
    check_label_is_the_design(cart_dir, &path, label, findings);
}

/// A cart that has a label design must ship what the design draws.
///
/// The designer saves `label.layers.json` on every edit; the PNG only moved on
/// an explicit export. A finished-looking design could sit beside the 96x96
/// scaffold placeholder and get published, which is what this catches.
fn check_label_is_the_design(
    cart_dir: &Path,
    art: &Path,
    label: &str,
    findings: &mut Vec<Finding>,
) {
    let doc = cart_dir.join(crate::labeldoc::DOC_FILE);
    if !doc.is_file() {
        return;
    }
    let bytes = match fs::read(art) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    if let Some((width, height)) = crate::labelart::png_dimensions(&bytes) {
        if (width, height)
            == (
                crate::labelart::PLACEHOLDER_SIZE,
                crate::labelart::PLACEHOLDER_SIZE,
            )
        {
            findings.push(warn_at(
                "CK003",
                format!(
                    "{} is still the {}x{} scaffold placeholder, but this cart has a label design; \
                     open the Label tab so the art is written",
                    python_repr(label),
                    crate::labelart::PLACEHOLDER_SIZE,
                    crate::labelart::PLACEHOLDER_SIZE
                ),
                label,
            ));
            return;
        }
    }
    let art_time = fs::metadata(art).and_then(|meta| meta.modified()).ok();
    let doc_time = fs::metadata(&doc).and_then(|meta| meta.modified()).ok();
    if let (Some(art_time), Some(doc_time)) = (art_time, doc_time) {
        if doc_time > art_time {
            findings.push(warn_at(
                "CK003",
                format!(
                    "the label design was edited after {} was written, so the cart would ship \
                     older art; open the Label tab to bring it up to date",
                    python_repr(label)
                ),
                label,
            ));
        }
    }
}

pub fn check_mods(cart: &Cart, findings: &mut Vec<Finding>) -> Vec<String> {
    let mods = match cart.get("mods") {
        Some(Value::Array(items)) if !items.is_empty() => items,
        _ => {
            findings.push(err(
                "CK004",
                "\"mods\" must list 1..64 pinned mods; a cart with no mods is just the base game",
            ));
            return Vec::new();
        }
    };
    if mods.len() > MAX_MODS {
        findings.push(err(
            "CK004",
            format!("\"mods\" has {} entries (max {})", mods.len(), MAX_MODS),
        ));
    }
    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut ids = Vec::new();
    for (index, entry) in mods.iter().enumerate() {
        let mut label = format!("mods[{}]", index + 1);
        let entry = match entry {
            Value::Object(map) => map,
            _ => {
                findings.push(err("CK004", format!("{} must be an object", label)));
                continue;
            }
        };
        let mod_id = entry.get("id");
        if !full_match(id_re(), mod_id) {
            findings.push(err(
                "CK004",
                format!(
                    "{} id must be 1..64 characters of letters, digits, _ or -",
                    label
                ),
            ));
        } else {
            let mod_id = mod_id
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            label = format!("mods[{}] {}", index + 1, mod_id);
            match seen.iter().find(|(seen_id, _)| *seen_id == mod_id) {
                Some((_, first)) => findings.push(err(
                    "CK004",
                    format!(
                        "{} repeats the id pinned at mods[{}]; one pin per mod",
                        label,
                        first + 1
                    ),
                )),
                None => {
                    seen.push((mod_id.clone(), index));
                    ids.push(mod_id);
                }
            }
        }
        match entry.get("source").and_then(Value::as_str) {
            Some("local") => findings.push(err(
                "CK004",
                format!(
                    "{} is pinned to one install; a published cart needs a github or gamebanana pin nobody else has to guess at",
                    label
                ),
            )),
            Some("github") => check_github_pin(entry, &label, findings),
            Some("gamebanana") => check_gamebanana_pin(entry, &label, findings),
            _ => findings.push(err(
                "CK004",
                format!("{} source must be {}", label, SOURCES.join(" or ")),
            )),
        }
        if let Some(enabled) = entry.get("enabled") {
            if !enabled.is_boolean() {
                findings.push(err(
                    "CK004",
                    format!(
                        "{} enabled must be true or false; omit it to ship the mod on",
                        label
                    ),
                ));
            }
        }
        if let Some(options) = entry.get("options").filter(|v| !v.is_null()) {
            for problem in option_problems(options, &label) {
                findings.push(err("CK004", problem));
            }
        }
        for key in entry.keys() {
            if !MOD_KEYS.contains(&key.as_str()) {
                findings.push(warn(
                    "CK004",
                    format!("{} has unknown field {}", label, python_repr(key)),
                ));
            }
        }
    }
    ids
}

fn check_github_pin(
    entry: &serde_json::Map<String, Value>,
    label: &str,
    findings: &mut Vec<Finding>,
) {
    if !full_match(repo_re(), entry.get("repo")) {
        findings.push(err("CK004", format!("{} repo must be owner/name", label)));
    } else if entry
        .get("repo")
        .and_then(Value::as_str)
        .map(|repo| repo.to_lowercase() == PLACEHOLDER_REPO)
        .unwrap_or(false)
    {
        findings.push(warn(
            "CK004",
            format!(
                "{} still points at the scaffold placeholder; pin a real release with cartkit pin <cart> owner/repo@X.Y.Z",
                label
            ),
        ));
    }
    if !full_match(semver_re(), entry.get("version")) {
        findings.push(err(
            "CK004",
            format!("{} version must be semver, e.g. 1.2.3", label),
        ));
    }
    let sha = entry.get("sha256");
    if !full_match(sha256_re(), sha) {
        findings.push(err(
            "CK004",
            format!(
                "{} sha256 must be 64 lowercase hex characters; cartkit pin resolves it for you",
                label
            ),
        ));
    } else if sha.and_then(Value::as_str) == Some(PLACEHOLDER_SHA) {
        findings.push(warn(
            "CK004",
            format!(
                "{} carries the scaffold placeholder hash; resolve it with cartkit pin",
                label
            ),
        ));
    }
    for key in ["mod", "file", "md5"] {
        if entry.contains_key(key) {
            findings.push(warn(
                "CK004",
                format!(
                    "{} has {}, which belongs to a gamebanana pin",
                    label,
                    python_repr(key)
                ),
            ));
        }
    }
}

fn check_gamebanana_pin(
    entry: &serde_json::Map<String, Value>,
    label: &str,
    findings: &mut Vec<Finding>,
) {
    for key in ["mod", "file"] {
        let value = entry.get(key);
        let good = is_int(value) && value.and_then(Value::as_i64).unwrap_or(0) > 0;
        if !good {
            findings.push(err(
                "CK004",
                format!("{} {} must be a positive integer id", label, key),
            ));
        }
    }
    if !full_match(md5_re(), entry.get("md5")) {
        findings.push(err(
            "CK004",
            format!(
                "{} md5 must be 32 lowercase hex characters; cartkit pin resolves it for you",
                label
            ),
        ));
    }
    for key in ["repo", "version", "sha256"] {
        if entry.contains_key(key) {
            findings.push(warn(
                "CK004",
                format!(
                    "{} has {}, which belongs to a github pin",
                    label,
                    python_repr(key)
                ),
            ));
        }
    }
}

pub fn check_load_order(cart: &Cart, ids: &[String], findings: &mut Vec<Finding>) {
    let order = match cart.get("load_order").filter(|v| !v.is_null()) {
        Some(value) => value,
        None => return,
    };
    let order = match order {
        Value::Array(items) if items.iter().all(Value::is_string) => items
            .iter()
            .map(|item| item.as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>(),
        _ => {
            findings.push(err(
                "CK005",
                "\"load_order\" must be an array of the mod ids",
            ));
            return;
        }
    };
    if ids.is_empty() {
        return;
    }
    let missing: Vec<&String> = ids.iter().filter(|id| !order.contains(id)).collect();
    let extra: Vec<&String> = order.iter().filter(|id| !ids.contains(id)).collect();
    let mut unique = order.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != order.len() {
        findings.push(err(
            "CK005",
            "\"load_order\" repeats an id; it is a permutation of the mods, not a list of passes",
        ));
    }
    if !missing.is_empty() {
        findings.push(err(
            "CK005",
            format!(
                "\"load_order\" leaves out {}; name every pinned mod or drop the field",
                join_refs(&missing)
            ),
        ));
    }
    if !extra.is_empty() {
        findings.push(err(
            "CK005",
            format!(
                "\"load_order\" names {}, which is not pinned in mods",
                join_refs(&extra)
            ),
        ));
    }
}

fn join_refs(items: &[&String]) -> String {
    items
        .iter()
        .map(|item| item.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Every offline rule, in cartkit's order.
pub fn schema_findings(cart: &Cart, cart_dir: Option<&Path>) -> Vec<Finding> {
    let mut findings = Vec::new();
    check_identity(cart, &mut findings);
    check_label(cart, cart_dir, &mut findings);
    let ids = check_mods(cart, &mut findings);
    check_load_order(cart, &ids, &mut findings);
    findings
}
