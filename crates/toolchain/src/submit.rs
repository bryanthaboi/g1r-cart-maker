//! Index submission. The contribution path is read from the index repo at
//! runtime, never hardcoded, and the plan is always handed back for review
//! before anything is created.

use crate::detect;
use crate::publish::{classify, Cause, CommandLog};
use crate::readiness::IndexHints;
use crate::runner::{CancelToken, Invocation, Output, RunError, Runner};
use cartcore::cart::{cart_str, mods_of, Cart};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const INDEX_REPO: &str = "bryanthaboi/gen1recomp-mod-index";
const TEMPLATE_DIR: &str = ".github/ISSUE_TEMPLATE";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssueTemplate {
    pub path: String,
    pub file: String,
    pub name: String,
    pub title: Option<String>,
    pub labels: Vec<String>,
    /// True for a YAML issue form; a plain markdown template has no fields.
    pub form: bool,
    pub fields: Vec<FormField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Discovery {
    pub repo: String,
    pub contributing: Option<String>,
    pub templates: Vec<IssueTemplate>,
    /// The data file a pull request would edit, when CONTRIBUTING names one.
    pub data_file: Option<String>,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionKind {
    IssueForm,
    Issue,
    PullRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrefilledField {
    pub id: String,
    pub label: String,
    pub value: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubmissionPlan {
    pub kind: SubmissionKind,
    pub repo: String,
    pub title: String,
    pub body: String,
    pub template: Option<String>,
    pub labels: Vec<String>,
    pub fields: Vec<PrefilledField>,
    pub data_file: Option<String>,
    pub branch: Option<String>,
    /// The index row a pull request would add, already in feed shape.
    pub entry: Option<Value>,
    pub guidance: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Submission {
    pub kind: SubmissionKind,
    pub url: Option<String>,
    pub commands: Vec<CommandLog>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmitError {
    pub step: String,
    pub cause: Cause,
    pub message: String,
    pub stderr: String,
    pub hint: &'static str,
    pub commands: Vec<CommandLog>,
}

impl std::fmt::Display for SubmitError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}: {}", self.step, self.message)
    }
}

impl std::error::Error for SubmitError {}

// ------- discovery

pub fn discover(
    runner: &dyn Runner,
    cancel: &CancelToken,
    repo: &str,
) -> Result<Discovery, RunError> {
    let mut discovery = Discovery {
        repo: repo.to_string(),
        ..Discovery::default()
    };

    let listing = api(
        runner,
        cancel,
        &format!("repos/{}/contents/{}", repo, TEMPLATE_DIR),
        false,
    )?;
    if listing.success() {
        let doc: Value = serde_json::from_str(&listing.stdout).unwrap_or(Value::Null);
        for entry in doc.as_array().cloned().unwrap_or_default() {
            let path = entry
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let file = entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !(file.ends_with(".yml") || file.ends_with(".yaml")) || file.starts_with("config.") {
                continue;
            }
            let raw = api(
                runner,
                cancel,
                &format!("repos/{}/contents/{}", repo, path),
                true,
            )?;
            if !raw.success() {
                discovery.problems.push(raw.problem());
                continue;
            }
            discovery
                .templates
                .push(parse_issue_form(path, file, &raw.stdout));
        }
    } else {
        discovery.problems.push(listing.problem());
    }

    for candidate in ["CONTRIBUTING.md", ".github/CONTRIBUTING.md"] {
        let doc = api(
            runner,
            cancel,
            &format!("repos/{}/contents/{}", repo, candidate),
            true,
        )?;
        if doc.success() {
            discovery.data_file = data_file_in(&doc.stdout);
            discovery.contributing = Some(doc.stdout);
            break;
        }
        discovery.problems.push(doc.problem());
    }
    Ok(discovery)
}

fn api(
    runner: &dyn Runner,
    cancel: &CancelToken,
    route: &str,
    raw: bool,
) -> Result<Output, RunError> {
    let accept = if raw {
        "Accept: application/vnd.github.raw"
    } else {
        "Accept: application/vnd.github+json"
    };
    runner.run(
        &Invocation::new(detect::GH, ["api", route, "-H", accept]),
        cancel,
    )
}

/// A minimal reader for a GitHub issue form: the header keys plus the id,
/// label and required flag of every body item.
pub fn parse_issue_form(path: &str, file: &str, body: &str) -> IssueTemplate {
    let mut template = IssueTemplate {
        path: path.to_string(),
        file: file.to_string(),
        name: file.to_string(),
        title: None,
        labels: Vec::new(),
        form: false,
        fields: Vec::new(),
    };
    let mut in_body = false;
    let mut current: Option<FormField> = None;
    for raw in body.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let top_level = !line.starts_with(' ') && !line.starts_with('-');
        if top_level {
            if let Some((key, value)) = split_key(trimmed) {
                match key {
                    "name" => template.name = unquote(value),
                    "title" => template.title = Some(unquote(value)),
                    "description" if template.title.is_none() => {}
                    "labels" => template.labels = list_of(value),
                    "body" => {
                        in_body = true;
                        template.form = true;
                    }
                    _ => in_body = false,
                }
                continue;
            }
        }
        if !in_body {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(field) = current.take() {
                template.fields.push(field);
            }
            let kind = split_key(rest)
                .filter(|(key, _)| *key == "type")
                .map(|(_, value)| unquote(value))
                .unwrap_or_default();
            current = Some(FormField {
                id: String::new(),
                label: String::new(),
                kind,
                required: false,
            });
            continue;
        }
        if let (Some(field), Some((key, value))) = (current.as_mut(), split_key(trimmed)) {
            match key {
                "id" => field.id = unquote(value),
                "label" if field.label.is_empty() => field.label = unquote(value),
                "required" => field.required = unquote(value) == "true",
                "type" if field.kind.is_empty() => field.kind = unquote(value),
                _ => {}
            }
        }
    }
    if let Some(field) = current {
        template.fields.push(field);
    }
    for field in &mut template.fields {
        if field.id.is_empty() {
            field.id = slug_of(&field.label);
        }
        if field.label.is_empty() {
            field.label = field.id.clone();
        }
    }
    template
        .fields
        .retain(|field| field.kind != "markdown" && !field.id.is_empty());
    template
}

fn split_key(line: &str) -> Option<(&str, &str)> {
    let at = line.find(':')?;
    let key = line[..at].trim();
    if key.is_empty() || key.contains(' ') {
        return None;
    }
    Some((key, line[at + 1..].trim()))
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches(['"', '\'']).to_string()
}

fn list_of(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(unquote)
        .filter(|item| !item.is_empty())
        .collect()
}

fn slug_of(label: &str) -> String {
    label
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// The first repository-relative data file CONTRIBUTING names.
pub fn data_file_in(contributing: &str) -> Option<String> {
    contributing
        .split(|ch: char| ch.is_whitespace() || ch == '`' || ch == '(' || ch == ')' || ch == '"')
        .map(|token| token.trim_matches([',', '.', ';', ':', '*']))
        .find(|token| {
            token.contains('/')
                && (token.ends_with(".json") || token.ends_with(".yml") || token.ends_with(".yaml"))
                && !token.contains("://")
        })
        .map(str::to_string)
}

// ------- the plan

pub fn plan(
    discovery: &Discovery,
    cart: &Cart,
    repo_slug: &str,
    hints: &IndexHints,
) -> SubmissionPlan {
    let id = cart_str(cart, "id").unwrap_or_default().to_string();
    let title = cart_str(cart, "title").unwrap_or(&id).to_string();
    let version = cart_str(cart, "version").unwrap_or_default().to_string();
    let entry = index_entry(cart, repo_slug, hints);
    let issue_title = format!("Add cart: {} {}", title, version);
    let mut guidance = Vec::new();

    let chosen = pick_template(discovery);
    if let Some(template) = chosen {
        let fields = prefill(template, cart, repo_slug, hints);
        guidance.push(format!(
            "{} is the index repo's own form; the values below are submitted as its sections.",
            template.file
        ));
        guidance.push("Review every field before submitting.".to_string());
        return SubmissionPlan {
            kind: if template.form {
                SubmissionKind::IssueForm
            } else {
                SubmissionKind::Issue
            },
            repo: discovery.repo.clone(),
            title: template
                .title
                .clone()
                .filter(|value| !value.is_empty())
                .map(|prefix| format!("{}{} {}", prefix, title, version))
                .unwrap_or(issue_title),
            body: render_body(&fields),
            template: Some(template.file.clone()),
            labels: template.labels.clone(),
            fields,
            data_file: None,
            branch: None,
            entry: Some(entry),
            guidance,
        };
    }

    if let Some(data_file) = &discovery.data_file {
        guidance.push(format!(
            "{} takes contributions as a pull request against {}.",
            discovery.repo, data_file
        ));
        guidance.push("The row below is added or replaced by cart id.".to_string());
        return SubmissionPlan {
            kind: SubmissionKind::PullRequest,
            repo: discovery.repo.clone(),
            title: issue_title,
            body: pr_body(cart, repo_slug),
            template: None,
            labels: Vec::new(),
            fields: default_fields(cart, repo_slug, hints),
            data_file: Some(data_file.clone()),
            branch: Some(format!("add-cart-{}", id)),
            entry: Some(entry),
            guidance,
        };
    }

    guidance.push(format!(
        "{} publishes no issue form and names no data file; this is filed as a plain issue.",
        discovery.repo
    ));
    let fields = default_fields(cart, repo_slug, hints);
    SubmissionPlan {
        kind: SubmissionKind::Issue,
        repo: discovery.repo.clone(),
        title: issue_title,
        body: render_body(&fields),
        template: None,
        labels: Vec::new(),
        fields,
        data_file: None,
        branch: None,
        entry: Some(entry),
        guidance,
    }
}

fn pick_template(discovery: &Discovery) -> Option<&IssueTemplate> {
    let scored = |template: &IssueTemplate| {
        let hay =
            format!("{} {} {}", template.file, template.name, template.path).to_ascii_lowercase();
        if hay.contains("cart") {
            3
        } else if hay.contains("submit") || hay.contains("index") || hay.contains("add") {
            2
        } else if template.form {
            1
        } else {
            0
        }
    };
    discovery
        .templates
        .iter()
        .filter(|template| scored(template) > 0)
        .max_by_key(|template| scored(template))
}

fn prefill(
    template: &IssueTemplate,
    cart: &Cart,
    repo_slug: &str,
    hints: &IndexHints,
) -> Vec<PrefilledField> {
    template
        .fields
        .iter()
        .map(|field| PrefilledField {
            id: field.id.clone(),
            label: if field.label.is_empty() {
                field.id.clone()
            } else {
                field.label.clone()
            },
            value: value_for(&field.id, &field.label, cart, repo_slug, hints),
            required: field.required,
        })
        .collect()
}

fn default_fields(cart: &Cart, repo_slug: &str, hints: &IndexHints) -> Vec<PrefilledField> {
    [
        ("repo", "Cart repository"),
        ("id", "Cart id"),
        ("title", "Title"),
        ("version", "Version"),
        ("author", "Author"),
        ("base", "Base game"),
        ("seal", "Seal"),
        ("summary", "Summary"),
        ("license", "License"),
        ("tags", "Tags"),
    ]
    .iter()
    .map(|(id, label)| PrefilledField {
        id: (*id).to_string(),
        label: (*label).to_string(),
        value: value_for(id, label, cart, repo_slug, hints),
        required: matches!(*id, "repo" | "id" | "title" | "version"),
    })
    .collect()
}

fn value_for(id: &str, label: &str, cart: &Cart, repo_slug: &str, hints: &IndexHints) -> String {
    let hay = format!("{} {}", id, label).to_ascii_lowercase();
    let has = |needle: &str| hay.contains(needle);
    let text = |key: &str| cart_str(cart, key).unwrap_or_default().to_string();

    if has("repo") || has("url") || has("link") {
        if has("url") || has("link") {
            return format!("https://github.com/{}", repo_slug);
        }
        return repo_slug.to_string();
    }
    if has("thumbnail") {
        return hints.thumbnail.clone().unwrap_or_default();
    }
    if has("license") {
        return hints.license.clone().unwrap_or_default();
    }
    if has("tag") {
        return hints.tags.join(", ");
    }
    if has("version") {
        return text("version");
    }
    if has("author") {
        return text("author");
    }
    if has("base") {
        return text("base");
    }
    if has("seal") {
        return text("seal");
    }
    if has("summary") || has("description") {
        return text("summary");
    }
    if has("title") || has("name") {
        return text("title");
    }
    if has("id") {
        return text("id");
    }
    String::new()
}

/// GitHub renders an issue form submission as one `### label` section per field.
pub fn render_body(fields: &[PrefilledField]) -> String {
    let mut body = String::new();
    for field in fields {
        body.push_str("### ");
        body.push_str(&field.label);
        body.push_str("\n\n");
        if field.value.trim().is_empty() {
            body.push_str("_No response_");
        } else {
            body.push_str(field.value.trim());
        }
        body.push_str("\n\n");
    }
    body.trim_end().to_string()
}

fn pr_body(cart: &Cart, repo_slug: &str) -> String {
    format!(
        "Adds `{}` {} to the index.\n\nRepository: https://github.com/{}\nRelease: https://github.com/{}/releases/tag/v{}\n",
        cart_str(cart, "id").unwrap_or_default(),
        cart_str(cart, "version").unwrap_or_default(),
        repo_slug,
        repo_slug,
        cart_str(cart, "version").unwrap_or_default()
    )
}

/// The cart as a schema_version 1 index row.
pub fn index_entry(cart: &Cart, repo_slug: &str, hints: &IndexHints) -> Value {
    let text = |key: &str| cart_str(cart, key).unwrap_or_default().to_string();
    let mut entry = Map::new();
    entry.insert("id".into(), json!(text("id")));
    entry.insert("title".into(), json!(text("title")));
    entry.insert("author".into(), json!(text("author")));
    entry.insert("version".into(), json!(text("version")));
    entry.insert("base".into(), json!(text("base")));
    entry.insert("seal".into(), json!(text("seal")));
    entry.insert("repo".into(), json!(repo_slug));
    for key in ["summary", "shell", "finish"] {
        let value = text(key);
        if !value.is_empty() {
            entry.insert(key.into(), json!(value));
        }
    }
    if let Some(speeds) = cart.get("speeds") {
        entry.insert("speeds".into(), speeds.clone());
    }
    let mut pins = Vec::new();
    for entry_value in mods_of(cart) {
        let mut pin = Map::new();
        for key in [
            "id", "source", "repo", "version", "sha256", "mod", "file", "md5",
        ] {
            if let Some(value) = entry_value.get(key) {
                pin.insert(key.to_string(), value.clone());
            }
        }
        pins.push(Value::Object(pin));
    }
    entry.insert("mods".into(), Value::Array(pins));
    if let Some(load_order) = cart.get("load_order") {
        entry.insert("load_order".into(), load_order.clone());
    }
    if !hints.tags.is_empty() {
        entry.insert("tags".into(), json!(hints.tags));
    }
    for (key, value) in [
        ("thumbnail", &hints.thumbnail),
        ("description_url", &hints.description_url),
        ("license", &hints.license),
        ("fixed_release_tag", &hints.fixed_release_tag),
    ] {
        if let Some(value) = value.as_ref().filter(|value| !value.is_empty()) {
            entry.insert(key.into(), json!(value));
        }
    }
    if let Some(automatic) = hints.automatic_version_check {
        entry.insert("automatic_version_check".into(), json!(automatic));
    }
    Value::Object(entry)
}

// ------- execution

struct Log {
    commands: Vec<CommandLog>,
}

impl Log {
    fn exec(
        &mut self,
        runner: &dyn Runner,
        cancel: &CancelToken,
        step: &str,
        invocation: &Invocation,
    ) -> Result<Output, SubmitError> {
        let output = runner
            .run(invocation, cancel)
            .map_err(|problem| SubmitError {
                step: step.to_string(),
                cause: match &problem {
                    RunError::NotFound(_) => Cause::ToolMissing,
                    _ => Cause::Unknown,
                },
                message: problem.to_string(),
                stderr: String::new(),
                hint: Cause::ToolMissing.hint(),
                commands: self.commands.clone(),
            })?;
        self.commands.push(CommandLog {
            argv: invocation.argv(),
            cwd: invocation.cwd.as_ref().map(|dir| dir.display().to_string()),
            code: output.code,
            stdout: output.stdout.clone(),
            stderr: output.stderr.clone(),
        });
        if output.cancelled {
            return Err(self.fail(step, Cause::Cancelled, "cancelled", String::new()));
        }
        if !output.success() {
            let problem = output.problem();
            return Err(self.fail(
                step,
                classify(&problem),
                format!("{} failed", step),
                problem,
            ));
        }
        Ok(output)
    }

    fn fail(
        &self,
        step: &str,
        cause: Cause,
        message: impl Into<String>,
        stderr: String,
    ) -> SubmitError {
        SubmitError {
            step: step.to_string(),
            cause,
            message: message.into(),
            stderr,
            hint: cause.hint(),
            commands: self.commands.clone(),
        }
    }
}

pub fn submit_issue(
    runner: &dyn Runner,
    cancel: &CancelToken,
    plan: &SubmissionPlan,
) -> Result<Submission, SubmitError> {
    let mut log = Log {
        commands: Vec::new(),
    };
    let mut args = vec![
        "issue".to_string(),
        "create".to_string(),
        "--repo".to_string(),
        plan.repo.clone(),
        "--title".to_string(),
        plan.title.clone(),
        "--body".to_string(),
        plan.body.clone(),
    ];
    for label in &plan.labels {
        args.push("--label".to_string());
        args.push(label.clone());
    }
    let output = log.exec(
        runner,
        cancel,
        "gh issue create",
        &Invocation::new(detect::GH, args),
    )?;
    Ok(Submission {
        kind: plan.kind,
        url: first_url(&output.stdout),
        commands: log.commands,
    })
}

/// Fork, branch, edit the data file, push, open the pull request.
pub fn submit_pull_request(
    runner: &dyn Runner,
    cancel: &CancelToken,
    plan: &SubmissionPlan,
    workdir: &Path,
) -> Result<Submission, SubmitError> {
    let mut log = Log {
        commands: Vec::new(),
    };
    let data_file = plan.data_file.clone().ok_or_else(|| {
        log.fail(
            "plan",
            Cause::Unknown,
            "the plan names no data file to edit",
            String::new(),
        )
    })?;
    let branch = plan
        .branch
        .clone()
        .unwrap_or_else(|| "add-cart".to_string());
    let entry = plan.entry.clone().ok_or_else(|| {
        log.fail(
            "plan",
            Cause::Unknown,
            "the plan carries no index row",
            String::new(),
        )
    })?;

    let account = log.exec(
        runner,
        cancel,
        "gh api user",
        &Invocation::new(detect::GH, ["api", "user", "--jq", ".login"]),
    )?;
    let login = account.stdout.trim().to_string();

    log.exec(
        runner,
        cancel,
        "gh repo fork",
        &Invocation::new(
            detect::GH,
            [
                "repo",
                "fork",
                plan.repo.as_str(),
                "--clone",
                "--remote=false",
            ],
        )
        .in_dir(workdir),
    )?;

    let name = plan.repo.rsplit('/').next().unwrap_or("index");
    let clone: PathBuf = workdir.join(name);

    log.exec(
        runner,
        cancel,
        "git checkout",
        &Invocation::new(detect::GIT, ["checkout", "-b", branch.as_str()]).in_dir(&clone),
    )?;

    let target = clone.join(&data_file);
    merge_entry(&target, &entry)
        .map_err(|problem| log.fail("edit data file", Cause::Io, problem, String::new()))?;

    log.exec(
        runner,
        cancel,
        "git add",
        &Invocation::new(detect::GIT, ["add", "--", data_file.as_str()]).in_dir(&clone),
    )?;
    log.exec(
        runner,
        cancel,
        "git commit",
        &Invocation::new(detect::GIT, ["commit", "-m", plan.title.as_str()]).in_dir(&clone),
    )?;
    log.exec(
        runner,
        cancel,
        "git push",
        &Invocation::new(detect::GIT, ["push", "-u", "origin", branch.as_str()]).in_dir(&clone),
    )?;

    let head = format!("{}:{}", login, branch);
    let output = log.exec(
        runner,
        cancel,
        "gh pr create",
        &Invocation::new(
            detect::GH,
            [
                "pr",
                "create",
                "--repo",
                plan.repo.as_str(),
                "--title",
                plan.title.as_str(),
                "--body",
                plan.body.as_str(),
                "--head",
                head.as_str(),
            ],
        )
        .in_dir(&clone),
    )?;

    Ok(Submission {
        kind: plan.kind,
        url: first_url(&output.stdout),
        commands: log.commands,
    })
}

/// Add or replace the row with this cart id, in whichever shape the file uses.
pub fn merge_entry(path: &Path, entry: &Value) -> Result<(), String> {
    let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut doc: Value = if existing.trim().is_empty() {
        Value::Array(Vec::new())
    } else {
        serde_json::from_str(&existing)
            .map_err(|problem| format!("{} is not readable JSON: {}", path.display(), problem))?
    };

    let rows = match &mut doc {
        Value::Array(rows) => rows,
        Value::Object(map) => {
            let key = ["carts", "mods", "entries"]
                .iter()
                .find(|key| map.get(**key).map(Value::is_array).unwrap_or(false))
                .copied()
                .unwrap_or("carts");
            map.entry(key.to_string())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .ok_or_else(|| format!("{} has no array of rows", path.display()))?
        }
        _ => {
            return Err(format!(
                "{} is neither a list nor an object",
                path.display()
            ))
        }
    };

    match rows
        .iter()
        .position(|row| row.get("id").and_then(Value::as_str) == Some(id))
    {
        Some(at) => rows[at] = entry.clone(),
        None => rows.push(entry.clone()),
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|problem| problem.to_string())?;
    }
    let mut body = serde_json::to_string_pretty(&doc).map_err(|problem| problem.to_string())?;
    body.push('\n');
    fs::write(path, body).map_err(|problem| problem.to_string())
}

fn first_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.starts_with("https://"))
        .map(|token| token.trim_end_matches(['.', ',']).to_string())
}
