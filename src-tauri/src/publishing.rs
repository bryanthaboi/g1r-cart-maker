//! Prepare GitHub Repo: a cancellable background run whose every step reports
//! progress, plus index readiness and index submission.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use toolchain::publish::{
    self, GitIdentity, PublishOptions, PublishOutcome, StepLog, StepState, StepUpdate,
};
use toolchain::readiness::{self, IndexHints, Readiness};
use toolchain::runner::{CancelToken, SystemRunner};
use toolchain::submit::{self, SubmissionKind, SubmissionPlan};

pub const PROGRESS_EVENT: &str = "publish://progress";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishRequest {
    pub dir: String,
    #[serde(default)]
    pub owner: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub commit_message: Option<String>,
    #[serde(default)]
    pub identity_name: Option<String>,
    #[serde(default)]
    pub identity_email: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressStep {
    pub id: String,
    pub label: String,
    pub state: StepState,
    pub detail: String,
    pub stages: Vec<publish::RunStage>,
    pub log: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishProgress {
    pub run_id: String,
    pub steps: Vec<ProgressStep>,
    pub current: Option<String>,
    pub done: bool,
    pub failed: bool,
    pub cancelled: bool,
    pub error: Option<String>,
    pub hint: Option<String>,
    pub repo_url: Option<String>,
    pub release_url: Option<String>,
    pub install_url: Option<String>,
    pub asset_name: Option<String>,
    pub tag: Option<String>,
    pub outcome: Option<PublishOutcome>,
}

impl PublishProgress {
    fn new(run_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            steps: publish::STEP_ORDER
                .iter()
                .map(|id| ProgressStep {
                    id: id.id().to_string(),
                    label: id.label().to_string(),
                    state: StepState::Pending,
                    detail: String::new(),
                    stages: Vec::new(),
                    log: String::new(),
                })
                .collect(),
            current: None,
            done: false,
            failed: false,
            cancelled: false,
            error: None,
            hint: None,
            repo_url: None,
            release_url: None,
            install_url: None,
            asset_name: None,
            tag: None,
            outcome: None,
        }
    }

    fn apply(&mut self, update: &StepUpdate) {
        self.current = Some(update.id.to_string());
        if let Some(slot) = self.steps.iter_mut().find(|step| step.id == update.id) {
            slot.state = update.state;
            slot.detail = update.detail.clone().unwrap_or_default();
            slot.stages = update.stages.clone();
        }
    }

    /// The finished run carries the command log for every step.
    fn absorb(&mut self, logs: &[StepLog]) {
        for log in logs {
            if let Some(slot) = self.steps.iter_mut().find(|step| step.id == log.id) {
                slot.state = log.state;
                slot.detail = log.detail.clone().unwrap_or_default();
                slot.log = crate::dto::render_log(&log.commands);
            }
        }
    }
}

struct Run {
    cancel: CancelToken,
    progress: Arc<Mutex<PublishProgress>>,
}

#[derive(Default)]
pub struct Runs {
    runs: Mutex<HashMap<String, Run>>,
}

impl Runs {
    pub fn state(&self, run_id: &str) -> Option<PublishProgress> {
        let runs = self.runs.lock().ok()?;
        let run = runs.get(run_id)?;
        run.progress.lock().ok().map(|guard| guard.clone())
    }

    pub fn cancel(&self, run_id: &str) -> bool {
        match self.runs.lock() {
            Ok(runs) => match runs.get(run_id) {
                Some(run) => {
                    run.cancel.cancel();
                    true
                }
                None => false,
            },
            Err(_) => false,
        }
    }
}

fn emit(app: &AppHandle, progress: &PublishProgress) {
    let _ = app.emit(PROGRESS_EVENT, progress);
}

pub fn start(app: AppHandle, runs: Arc<Runs>, request: PublishRequest) -> AppResult<String> {
    let dir = PathBuf::from(&request.dir);
    if !dir.is_dir() {
        return Err(AppError::not_found("that cart directory is gone"));
    }
    if request.name.trim().is_empty() {
        return Err(AppError::invalid("a repo needs a name"));
    }
    let owner = match request
        .owner
        .as_deref()
        .map(str::trim)
        .filter(|o| !o.is_empty())
    {
        Some(owner) => owner.to_string(),
        None => authenticated_account()?,
    };
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancel = CancelToken::new();
    let progress = Arc::new(Mutex::new(PublishProgress::new(&run_id)));
    {
        let mut guard = runs
            .runs
            .lock()
            .map_err(|_| AppError::new("io", "the publish registry is locked"))?;
        guard.insert(
            run_id.clone(),
            Run {
                cancel: cancel.clone(),
                progress: Arc::clone(&progress),
            },
        );
    }

    let identity = match (
        request.identity_name.clone(),
        request.identity_email.clone(),
    ) {
        (Some(name), Some(email)) if !name.trim().is_empty() && !email.trim().is_empty() => {
            Some(GitIdentity { name, email })
        }
        _ => None,
    };
    let mut options = PublishOptions::new(dir, owner, request.name.trim());
    options.private = request.is_private;
    options.identity = identity;
    if !request.description.trim().is_empty() {
        options.description = Some(request.description.trim().to_string());
    }
    if let Some(message) = request.commit_message.clone() {
        if !message.trim().is_empty() {
            options.commit_message = message;
        }
    }

    let thread_progress = Arc::clone(&progress);
    let thread_app = app.clone();
    std::thread::spawn(move || {
        let runner = SystemRunner;
        let report = |update: &StepUpdate| {
            if let Ok(mut guard) = thread_progress.lock() {
                guard.apply(update);
                emit(&thread_app, &guard);
            }
        };
        let outcome = publish::publish(&runner, &cancel, &options, &report);
        if let Ok(mut guard) = thread_progress.lock() {
            match outcome {
                Ok(outcome) => {
                    guard.done = true;
                    guard.current = None;
                    guard.absorb(&outcome.steps);
                    guard.repo_url = Some(outcome.repo_url.clone());
                    guard.release_url = outcome.release_url.clone();
                    guard.install_url = outcome.install_url.clone();
                    guard.asset_name = Some(outcome.asset.clone());
                    guard.tag = Some(outcome.tag.clone());
                    guard.outcome = Some(outcome);
                }
                Err(problem) => {
                    guard.done = true;
                    guard.failed = true;
                    guard.cancelled = matches!(problem.cause, publish::Cause::Cancelled);
                    guard.error = Some(problem.message.clone());
                    guard.hint = Some(problem.hint.to_string());
                    guard.absorb(&problem.steps);
                }
            }
            emit(&thread_app, &guard);
        }
    });

    Ok(run_id)
}

/// Without an explicit owner the repo lands under whoever gh is logged in as.
fn authenticated_account() -> AppResult<String> {
    let runner = SystemRunner;
    let cancel = CancelToken::new();
    let auth = toolchain::detect::gh_auth_status(
        &runner,
        &cancel,
        toolchain::detect::TokenEnv::from_env(),
    )
    .map_err(|problem| AppError::new("gh", problem.to_string()))?;
    auth.account.ok_or_else(|| {
        AppError::invalid("gh did not name an account; type the owner for the new repository")
    })
}

fn cart_of(dir: &Path) -> AppResult<cartcore::Cart> {
    cartcore::read_cart(dir).map_err(|problem| AppError::invalid(problem.to_string()))
}

/// Remote facts come from gh; without a repo the local half is still evaluated.
/// Readiness and the submission read the sidecar first, then the cart.
fn hints_for(dir: &Path, cart: &cartcore::Cart) -> IndexHints {
    let entry = cartcore::indexentry::read(dir).over(cart);
    IndexHints {
        thumbnail: entry.thumbnail,
        description_url: entry.description_url,
        license: entry.license,
        tags: entry.tags,
        automatic_version_check: entry.automatic_version_check,
        fixed_release_tag: entry.fixed_release_tag,
    }
}

pub fn index_readiness(dir: &Path) -> AppResult<Readiness> {
    let cart = cart_of(dir)?;
    let hints = hints_for(dir, &cart);
    let slug = cart
        .get("repo")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    match slug {
        Some(slug) => {
            let runner = SystemRunner;
            let cancel = CancelToken::new();
            match readiness::check(&runner, &cancel, &cart, &slug, &hints) {
                Ok(readiness) => Ok(readiness),
                Err(_) => Ok(readiness::evaluate(&cart, None, &hints)),
            }
        }
        None => Ok(readiness::evaluate(&cart, None, &hints)),
    }
}

pub fn submission_plan(state: &AppState, dir: &Path) -> AppResult<SubmissionPlan> {
    let cart = cart_of(dir)?;
    let hints = hints_for(dir, &cart);
    let slug = cart
        .get("repo")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::invalid("set the cart's repo before submitting it to an index"))?
        .to_string();
    let index_repo = state
        .settings()
        .index_sources
        .iter()
        .find(|source| source.builtin)
        .map(|source| source.url.clone())
        .unwrap_or_else(|| crate::settings::BUILTIN_SOURCE_URL.to_string());
    let runner = SystemRunner;
    let cancel = CancelToken::new();
    let discovery = submit::discover(&runner, &cancel, &index_repo)
        .map_err(|problem| AppError::network(problem.to_string()))?;
    Ok(submit::plan(&discovery, &cart, &slug, &hints))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionResult {
    pub kind: SubmissionKind,
    pub url: Option<String>,
    pub commands: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionEdits {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub fields: Vec<FieldEdit>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldEdit {
    pub id: String,
    pub value: String,
}

/// The plan is always re-derived here and the user's edits applied on top, so
/// nothing the window sends can become the submission on its own.
pub fn submit(
    state: &AppState,
    dir: &Path,
    edits: &SubmissionEdits,
) -> AppResult<SubmissionResult> {
    let mut plan = submission_plan(state, dir)?;
    for edit in &edits.fields {
        if let Some(field) = plan.fields.iter_mut().find(|field| field.id == edit.id) {
            field.value = edit.value.clone();
        }
    }
    if !edits.fields.is_empty() {
        plan.body = submit::render_body(&plan.fields);
    }
    if let Some(title) = edits.title.as_ref().filter(|text| !text.trim().is_empty()) {
        plan.title = title.trim().to_string();
    }
    if let Some(body) = edits.body.as_ref().filter(|text| !text.trim().is_empty()) {
        plan.body = body.clone();
    }
    let runner = SystemRunner;
    let cancel = CancelToken::new();
    let outcome = match plan.kind {
        SubmissionKind::Issue | SubmissionKind::IssueForm => {
            submit::submit_issue(&runner, &cancel, &plan)
        }
        SubmissionKind::PullRequest => {
            let workdir = state.paths.cache.join("submissions");
            std::fs::create_dir_all(&workdir)?;
            submit::submit_pull_request(&runner, &cancel, &plan, &workdir)
        }
    };
    let outcome = outcome.map_err(|problem| {
        AppError::new("gh", problem.to_string()).with_detail(problem.stderr.clone())
    })?;
    Ok(SubmissionResult {
        kind: outcome.kind,
        url: outcome.url,
        commands: outcome.commands.iter().map(|c| c.argv.clone()).collect(),
    })
}
