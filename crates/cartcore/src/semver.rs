//! Versions and the range grammar, ported from `src/mods/Semver.lua`:
//! `> >= < <= ^`, bare means `=`, spaces AND, `||` alternates.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Lua's `tonumber` for strings: decimal, hex, exponents, leading sign.
pub fn lua_tonumber(text: &str) -> Option<f64> {
    let body = text.trim();
    if body.is_empty() {
        return None;
    }
    let (negative, rest) = match body.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, body.strip_prefix('+').unwrap_or(body)),
    };
    if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let value = u64::from_str_radix(hex, 16).ok()? as f64;
        return Some(if negative { -value } else { value });
    }
    if rest
        .bytes()
        .any(|b| b.is_ascii_alphabetic() && b != b'e' && b != b'E')
    {
        return None;
    }
    let value = rest.parse::<f64>().ok().filter(|v| v.is_finite())?;
    Some(if negative { -value } else { value })
}

/// Absent components are 0; build metadata is parsed then discarded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl Version {
    /// `None` for anything unparsable; mod versions stay free-form strings.
    pub fn parse(text: &str) -> Option<Version> {
        let body = text.trim();
        let body = body.strip_prefix(|c| c == 'v' || c == 'V').unwrap_or(body);
        let body = match body.find('+') {
            Some(at) => &body[..at],
            None => body,
        };
        let (core, pre) = match body.find('-') {
            Some(0) => return None,
            Some(at) => (&body[..at], &body[at + 1..]),
            None => (body, ""),
        };
        if core.is_empty()
            || !core.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            || core.starts_with('.')
            || core.ends_with('.')
            || core.contains("..")
        {
            return None;
        }
        let mut nums = Vec::new();
        for part in core.split('.') {
            nums.push(part.parse::<u64>().ok()?);
        }
        if nums.is_empty() || nums.len() > 3 {
            return None;
        }
        let pre = if pre.is_empty() {
            None
        } else if pre
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        {
            Some(pre.to_string())
        } else {
            return None;
        };
        Some(Version {
            major: nums[0],
            minor: nums.get(1).copied().unwrap_or(0),
            patch: nums.get(2).copied().unwrap_or(0),
            pre,
        })
    }
}

/// SemVer 2.0 pre-release precedence: a release outranks its pre-releases,
/// numeric identifiers compare numerically and rank below alphanumeric ones.
fn compare_pre(a: Option<&str>, b: Option<&str>) -> Ordering {
    let (a, b) = match (a, b) {
        (None, None) => return Ordering::Equal,
        (None, Some(_)) => return Ordering::Greater,
        (Some(_), None) => return Ordering::Less,
        (Some(x), Some(y)) if x == y => return Ordering::Equal,
        (Some(x), Some(y)) => (x, y),
    };
    let left: Vec<&str> = a.split('.').filter(|p| !p.is_empty()).collect();
    let right: Vec<&str> = b.split('.').filter(|p| !p.is_empty()).collect();
    let count = left.len().max(right.len());
    for i in 0..count {
        let (x, y) = match (left.get(i), right.get(i)) {
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) => (*x, *y),
        };
        match (lua_tonumber(x), lua_tonumber(y)) {
            (Some(nx), Some(ny)) => {
                if nx != ny {
                    return if nx < ny {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                }
            }
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => {
                if x != y {
                    return x.cmp(y);
                }
            }
        }
    }
    Ordering::Equal
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then_with(|| compare_pre(self.pre.as_deref(), other.pre.as_deref()))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

/// `None` when either side is unparsable.
pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    Some(Version::parse(a)?.cmp(&Version::parse(b)?))
}

const OPS: [&str; 7] = ["=", "==", ">", ">=", "<", "<=", "^"];

/// `^` pins the leftmost non-zero component: `^1.2` is `>=1.2 <2.0`.
fn caret_upper(v: &Version) -> Version {
    let (major, minor, patch) = if v.major > 0 {
        (v.major + 1, 0, 0)
    } else if v.minor > 0 {
        (0, v.minor + 1, 0)
    } else {
        (0, 0, v.patch + 1)
    };
    Version {
        major,
        minor,
        patch,
        pre: None,
    }
}

fn match_token(version: &Version, token: &str) -> Result<bool, String> {
    let head = token.len() - token.trim_start_matches(['=', '<', '>', '^']).len();
    let (op, rest) = token.split_at(head);
    let op = if op.is_empty() { "=" } else { op };
    if !OPS.contains(&op) {
        return Err(format!("unknown comparator {:?} in range", op));
    }
    let target = match Version::parse(rest) {
        Some(target) => target,
        None => return Err(format!("unparsable version {:?} in range", rest)),
    };
    let order = version.cmp(&target);
    Ok(match op {
        "=" | "==" => order == Ordering::Equal,
        ">" => order == Ordering::Greater,
        ">=" => order != Ordering::Less,
        "<" => order == Ordering::Less,
        "<=" => order != Ordering::Greater,
        _ => order != Ordering::Less && version.cmp(&caret_upper(&target)) == Ordering::Less,
    })
}

/// True only when every space-separated comparator in one alternative holds.
fn match_alternative(version: &Version, alternative: &str) -> Result<bool, String> {
    let mut tokens = 0;
    let mut ok = true;
    for token in alternative.split_whitespace() {
        tokens += 1;
        if !match_token(version, token)? {
            ok = false;
        }
    }
    if tokens == 0 {
        return Err("empty range alternative".to_string());
    }
    Ok(ok)
}

/// False with no reason for a clean miss, false plus a reason for an
/// unparsable version or a malformed range.
pub fn satisfies_reason(version: &str, range: &str) -> (bool, Option<String>) {
    let parsed = match Version::parse(version) {
        Some(parsed) => parsed,
        None => return (false, Some(format!("unparsable version {:?}", version))),
    };
    if range.is_empty() {
        return (true, None);
    }
    let mut matched = false;
    for alternative in range.split("||") {
        match match_alternative(&parsed, alternative) {
            Err(err) => return (false, Some(err)),
            Ok(ok) => matched = matched || ok,
        }
    }
    (matched, None)
}

pub fn satisfies(version: &str, range: &str) -> bool {
    satisfies_reason(version, range).0
}

/// Grammar-only check, for manifest validation where no version is in hand.
pub fn valid_range(range: &str) -> Result<(), String> {
    if range.is_empty() {
        return Ok(());
    }
    match satisfies_reason("0.0.0", range).1 {
        Some(err) => Err(err),
        None => Ok(()),
    }
}
