//! New Cart: the directory cartkit scaffold writes, file for file.

use crate::cart::{write_cart, Cart};
use crate::labelart::label_art;
use crate::schema::*;
use crate::workflow::{render, WorkflowOptions, WORKFLOW_PATH};
use serde_json::{json, Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const README_TEMPLATE: &str = include_str!("../templates/README.md");
pub const CHANGELOG_TEMPLATE: &str = include_str!("../templates/CHANGELOG.md");
pub const GITIGNORE_TEMPLATE: &str = include_str!("../templates/gitignore");
pub const DEFAULT_LABEL: &str = "label.png";

#[derive(Debug, Clone)]
pub struct ScaffoldOptions {
    pub id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub base: String,
    pub shell: Option<String>,
    pub seal: String,
    pub github: Option<String>,
    /// Engine release the cart's `engine` range is built from.
    pub engine: String,
    pub force: bool,
}

impl ScaffoldOptions {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            author: None,
            summary: None,
            base: "red".to_string(),
            shell: None,
            seal: "sealed".to_string(),
            github: None,
            engine: "0.0.0-dev".to_string(),
            force: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScaffoldError {
    #[error("bad id {0} (letters, digits, _ or -, 1..64 characters)")]
    BadId(String),
    #[error("bad shell {0} (expected #rrggbb)")]
    BadShell(String),
    #[error("bad --github {0} (expected owner/name or a github.com URL)")]
    BadGithub(String),
    #[error("bad title {0} (1..48 characters)")]
    BadTitle(String),
    #[error("base {0} is not one of red, blue, yellow, gold, silver, crystal")]
    BadBase(String),
    #[error("seal {0} is not one of sealed, sealed+, open")]
    BadSeal(String),
    #[error("{0} exists")]
    Exists(PathBuf),
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// Python's str.title(): each run of letters capitalized, the rest lowered.
fn title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_alpha = false;
    for ch in text.chars() {
        if ch.is_alphabetic() {
            if previous_alpha {
                out.extend(ch.to_lowercase());
            } else {
                out.extend(ch.to_uppercase());
            }
            previous_alpha = true;
        } else {
            out.push(ch);
            previous_alpha = false;
        }
    }
    out
}

pub fn default_title(id: &str) -> String {
    title_case(&id.replace(['_', '-'], " "))
}

/// `>=<engine> <major+1>.0.0`, the range cartkit scaffolds.
pub fn engine_range(engine: &str) -> String {
    let major: u64 = engine
        .split('.')
        .next()
        .and_then(|head| head.parse().ok())
        .unwrap_or(0);
    format!(">={} <{}.0.0", engine, major + 1)
}

pub fn scaffold_cart(options: &ScaffoldOptions) -> Result<Cart, ScaffoldError> {
    if !id_re().is_match(&options.id) {
        return Err(ScaffoldError::BadId(crate::validate::python_repr(
            &options.id,
        )));
    }
    if let Some(shell) = &options.shell {
        if !shell_re().is_match(shell) {
            return Err(ScaffoldError::BadShell(crate::validate::python_repr(shell)));
        }
    }
    if !is_base(&options.base) {
        return Err(ScaffoldError::BadBase(options.base.clone()));
    }
    if !is_seal(&options.seal) {
        return Err(ScaffoldError::BadSeal(options.seal.clone()));
    }
    let github = match &options.github {
        Some(text) if !text.trim().is_empty() => Some(
            crate::spec::parse_slug(text)
                .ok_or_else(|| ScaffoldError::BadGithub(crate::validate::python_repr(text)))?,
        ),
        _ => None,
    };
    let title = options
        .title
        .clone()
        .unwrap_or_else(|| default_title(&options.id));
    if crate::validate::text_problem(Some(&Value::String(title.clone())), 1, MAX_TITLE).is_some() {
        return Err(ScaffoldError::BadTitle(crate::validate::python_repr(
            &title,
        )));
    }
    let shell = options
        .shell
        .clone()
        .unwrap_or_else(|| DEFAULT_SHELL.to_string())
        .to_lowercase();

    let mut cart = Map::new();
    cart.insert("schema".into(), json!(CART_SCHEMA));
    cart.insert("id".into(), json!(options.id));
    cart.insert("title".into(), json!(title));
    cart.insert("version".into(), json!("0.1.0"));
    cart.insert(
        "author".into(),
        json!(options
            .author
            .clone()
            .unwrap_or_else(|| "TODO your handle".into())),
    );
    let summary = options.summary.clone().unwrap_or_default();
    if !summary.is_empty() {
        cart.insert("summary".into(), json!(summary));
    }
    cart.insert("shell".into(), json!(shell));
    cart.insert("label".into(), json!(DEFAULT_LABEL));
    cart.insert("base".into(), json!(options.base));
    cart.insert("engine".into(), json!(engine_range(&options.engine)));
    cart.insert("seal".into(), json!(options.seal));
    cart.insert(
        "mods".into(),
        json!([{
            "id": "example-mod",
            "source": "github",
            "repo": PLACEHOLDER_REPO,
            "version": "0.1.0",
            "sha256": PLACEHOLDER_SHA,
        }]),
    );
    if let Some(github) = github {
        cart.insert("repo".into(), json!(github));
    }
    Ok(crate::cart::ordered(&cart))
}

fn substitute(template: &str, cart: &Cart) -> String {
    let get = |key: &str| {
        cart.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    template
        .replace("{{id}}", &get("id"))
        .replace("{{title}}", &get("title"))
        .replace("{{base}}", &get("base"))
        .replace("{{seal}}", &get("seal"))
        .replace("{{label}}", &get("label"))
}

/// Write a whole cart directory: manifest, placeholder art, docs and workflow.
pub fn scaffold_into(dest: &Path, options: &ScaffoldOptions) -> Result<Cart, ScaffoldError> {
    if dest.exists() && !options.force {
        return Err(ScaffoldError::Exists(dest.to_path_buf()));
    }
    let cart = scaffold_cart(options)?;
    fs::create_dir_all(dest)?;
    write_cart(dest, &cart)?;
    let shell = cart
        .get("shell")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_SHELL);
    fs::write(dest.join(DEFAULT_LABEL), label_art(shell))?;
    fs::write(dest.join("README.md"), substitute(README_TEMPLATE, &cart))?;
    fs::write(
        dest.join("CHANGELOG.md"),
        substitute(CHANGELOG_TEMPLATE, &cart),
    )?;
    fs::write(dest.join(".gitignore"), GITIGNORE_TEMPLATE)?;
    write_workflow(dest, &options.id)?;
    Ok(cart)
}

pub fn write_workflow(dest: &Path, cart_id: &str) -> io::Result<()> {
    let path = dest.join(WORKFLOW_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, render(&WorkflowOptions::new(cart_id)))
}
