//! Every Tauri command. Each one is a thin shell over a library crate.
//!
//! Anything that touches the network, a subprocess or more than one file runs
//! through `off_thread`: a plain `fn` command runs on the main thread, so a
//! blocking call there freezes the window.

use crate::error::{AppError, AppResult};
use crate::label::LabelTemplate;
use crate::options::OptionDiscovery;
use crate::project::{ExportResult, ProjectState, ScaffoldRequest};
use crate::settings::{IndexSourceSetting, Settings};
use crate::state::AppState;
use cartcore::labeldoc::{ExportCheck, LabelDoc};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;

fn dir_of(dir: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(dir);
    if !path.is_dir() {
        return Err(AppError::not_found(format!("{} is not a directory", dir)));
    }
    Ok(path)
}

/// Run blocking work on the pool and keep the main thread free for the UI.
async fn off_thread<T, F>(work: F) -> AppResult<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| AppError::new("io", "that task stopped before it finished"))
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> AppResult<Settings> {
    Ok(state.settings())
}

#[tauri::command]
pub fn save_settings(state: State<'_, Arc<AppState>>, next: Settings) -> AppResult<Settings> {
    state.replace_settings(next)
}

#[tauri::command]
pub async fn cache_usage(state: State<'_, Arc<AppState>>) -> AppResult<crate::state::CacheUsage> {
    let state = state.inner().clone();
    off_thread(move || state.cache_usage()).await
}

#[tauri::command]
pub async fn clear_cache(
    state: State<'_, Arc<AppState>>,
    kind: String,
) -> AppResult<crate::state::CacheUsage> {
    let state = state.inner().clone();
    off_thread(move || state.clear_cache(&kind)).await?
}

#[tauri::command]
pub async fn export_app_data(
    state: State<'_, Arc<AppState>>,
    out_path: String,
) -> AppResult<String> {
    let state = state.inner().clone();
    off_thread(move || state.export_data(Path::new(&out_path))).await?
}

#[tauri::command]
pub fn forget_project(state: State<'_, Arc<AppState>>, path: String) -> AppResult<Settings> {
    state.mutate_settings(|settings| {
        settings
            .recent_projects
            .retain(|recent| recent.path != path);
        Ok(())
    })
}

#[tauri::command]
pub async fn scaffold_project(
    state: State<'_, Arc<AppState>>,
    request: ScaffoldRequest,
) -> AppResult<ProjectState> {
    let state = state.inner().clone();
    off_thread(move || {
        let mut settings = state.settings();
        let project = crate::project::scaffold(&request, &mut settings)?;
        state.replace_settings(settings)?;
        Ok(project)
    })
    .await?
}

#[tauri::command]
pub async fn open_project(state: State<'_, Arc<AppState>>, dir: String) -> AppResult<ProjectState> {
    let state = state.inner().clone();
    off_thread(move || {
        let path = dir_of(&dir)?;
        let mut settings = state.settings();
        let project = crate::project::open(&path, &mut settings)?;
        state.replace_settings(settings)?;
        Ok(project)
    })
    .await?
}

#[tauri::command]
pub async fn save_project(dir: String, cart: Value) -> AppResult<ProjectState> {
    off_thread(move || crate::project::save(&dir_of(&dir)?, cart)).await?
}

#[tauri::command]
pub async fn validate_project(dir: String) -> AppResult<cartcore::Report> {
    off_thread(move || crate::project::validate(&dir_of(&dir)?)).await?
}

#[tauri::command]
pub async fn bundle_name(dir: String) -> AppResult<String> {
    off_thread(move || crate::project::default_bundle_name(&dir_of(&dir)?)).await?
}

#[tauri::command]
pub async fn export_bundle(dir: String, out_path: String) -> AppResult<ExportResult> {
    off_thread(move || crate::project::export_bundle(&dir_of(&dir)?, Path::new(&out_path))).await?
}

#[tauri::command]
pub async fn write_workflow(dir: String) -> AppResult<ProjectState> {
    off_thread(move || {
        let path = dir_of(&dir)?;
        let cart =
            cartcore::read_cart(&path).map_err(|problem| AppError::invalid(problem.to_string()))?;
        let id = cart
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::invalid("this cart has no id yet"))?;
        cartcore::scaffold::write_workflow(&path, id)?;
        crate::project::state(&path)
    })
    .await?
}

#[tauri::command]
pub async fn add_pin(dir: String, pin: Value) -> AppResult<ProjectState> {
    off_thread(move || crate::project::add_pin(&dir_of(&dir)?, pin)).await?
}

#[tauri::command]
pub async fn remove_pin(dir: String, id: String) -> AppResult<ProjectState> {
    off_thread(move || crate::project::remove_pin(&dir_of(&dir)?, &id)).await?
}

#[tauri::command]
pub async fn reorder_pins(dir: String, order: Vec<String>) -> AppResult<ProjectState> {
    off_thread(move || crate::project::reorder_pins(&dir_of(&dir)?, order)).await?
}

#[tauri::command]
pub async fn set_pin_options(dir: String, id: String, options: Value) -> AppResult<ProjectState> {
    off_thread(move || crate::project::set_pin_options(&dir_of(&dir)?, &id, options)).await?
}

#[tauri::command]
pub async fn set_pin_enabled(dir: String, id: String, enabled: bool) -> AppResult<ProjectState> {
    off_thread(move || crate::project::set_pin_enabled(&dir_of(&dir)?, &id, enabled)).await?
}

#[tauri::command]
pub async fn label_templates() -> AppResult<Vec<LabelTemplate>> {
    off_thread(crate::label::templates).await
}

#[tauri::command]
pub async fn read_label_doc(dir: String) -> AppResult<Option<LabelDoc>> {
    off_thread(move || Ok(crate::project::read_label_doc(&dir_of(&dir)?))).await?
}

#[tauri::command]
pub async fn write_label_doc(dir: String, doc: LabelDoc) -> AppResult<()> {
    off_thread(move || crate::project::write_label_doc(&dir_of(&dir)?, &doc)).await?
}

#[tauri::command]
pub async fn check_label_export(data_url: String, label_path: String) -> AppResult<ExportCheck> {
    off_thread(move || crate::label::check(&data_url, &label_path)).await?
}

#[tauri::command]
pub async fn write_label_png(
    dir: String,
    label_path: String,
    data_url: String,
) -> AppResult<ExportCheck> {
    off_thread(move || crate::label::write_png(&dir_of(&dir)?, &label_path, &data_url)).await?
}

#[tauri::command]
pub async fn read_image_data_url(path: String) -> AppResult<String> {
    off_thread(move || crate::label::read_image_data_url(Path::new(&path))).await?
}

#[tauri::command]
pub async fn placeholder_label(shell: String) -> AppResult<String> {
    off_thread(move || crate::label::placeholder(&shell)).await?
}

#[tauri::command]
pub async fn mod_options_from_install(
    save_dir: String,
) -> AppResult<HashMap<String, OptionDiscovery>> {
    off_thread(move || crate::options::from_install(Path::new(&save_dir))).await?
}

#[tauri::command]
pub fn index_sources(state: State<'_, Arc<AppState>>) -> Vec<IndexSourceSetting> {
    state.settings().index_sources
}

#[tauri::command]
pub fn add_index_source(
    state: State<'_, Arc<AppState>>,
    url: String,
) -> AppResult<Vec<IndexSourceSetting>> {
    let resolved = cartcore::index::resolve_source(&url).map_err(AppError::invalid)?;
    let id = crate::state::source_id(&resolved.feed);
    let settings = state.mutate_settings(|settings| {
        if settings.index_sources.iter().any(|source| source.id == id) {
            return Err(AppError::invalid("that index is already listed"));
        }
        settings.index_sources.push(IndexSourceSetting {
            id: id.clone(),
            url: url.clone(),
            feed: resolved.feed.clone(),
            base: resolved.base.clone(),
            fallback: resolved.fallback.clone(),
            label: resolved.label.clone(),
            enabled: true,
            builtin: false,
        });
        Ok(())
    })?;
    Ok(settings.index_sources)
}

#[tauri::command]
pub fn remove_index_source(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<Vec<IndexSourceSetting>> {
    let settings = state.mutate_settings(|settings| {
        if settings
            .index_sources
            .iter()
            .any(|source| source.id == id && source.builtin)
        {
            settings.builtin_removed = true;
        }
        settings.index_sources.retain(|source| source.id != id);
        Ok(())
    })?;
    Ok(settings.index_sources)
}

#[tauri::command]
pub async fn resolve_spec(
    state: State<'_, Arc<AppState>>,
    spec: String,
    mod_id: Option<String>,
    file_id: Option<u64>,
) -> AppResult<crate::network::Resolution> {
    let state = state.inner().clone();
    off_thread(move || crate::network::resolve_spec(&state, &spec, mod_id.as_deref(), file_id))
        .await?
}

#[tauri::command]
pub async fn github_releases(
    state: State<'_, Arc<AppState>>,
    slug: String,
) -> AppResult<Vec<crate::dto::Release>> {
    let state = state.inner().clone();
    off_thread(move || {
        Ok(crate::network::github_releases(&state, &slug)?
            .iter()
            .map(crate::dto::Release::from)
            .collect())
    })
    .await?
}

#[tauri::command]
pub async fn gamebanana_files(
    state: State<'_, Arc<AppState>>,
    mod_id: u64,
) -> AppResult<Vec<crate::dto::GameBananaFile>> {
    let state = state.inner().clone();
    off_thread(move || {
        Ok(crate::network::gamebanana_files(&state, mod_id)?
            .iter()
            .map(crate::dto::GameBananaFile::from)
            .collect())
    })
    .await?
}

#[tauri::command]
pub async fn validate_online(
    state: State<'_, Arc<AppState>>,
    dir: String,
) -> AppResult<cartcore::Report> {
    let state = state.inner().clone();
    off_thread(move || crate::network::validate_online(&state, &dir_of(&dir)?, true)).await?
}

#[tauri::command]
pub async fn fetch_index(
    state: State<'_, Arc<AppState>>,
    source_id: String,
    refresh: bool,
) -> AppResult<crate::network::IndexFeed> {
    let state = state.inner().clone();
    off_thread(move || crate::network::fetch_index(&state, &source_id, refresh)).await?
}

#[tauri::command]
pub async fn fetch_thumbnail(state: State<'_, Arc<AppState>>, url: String) -> AppResult<String> {
    let state = state.inner().clone();
    off_thread(move || crate::network::fetch_thumbnail(&state, &url)).await?
}

#[tauri::command]
pub async fn mod_options_from_archive(
    state: State<'_, Arc<AppState>>,
    pin: Value,
) -> AppResult<OptionDiscovery> {
    let state = state.inner().clone();
    off_thread(move || crate::network::options_from_archive(&state, pin)).await?
}

#[tauri::command]
pub async fn app_environment(
    state: State<'_, Arc<AppState>>,
    dir: Option<String>,
) -> AppResult<crate::env::Environment> {
    let state = state.inner().clone();
    off_thread(move || crate::env::environment(&state, dir.as_deref().map(Path::new))).await
}

#[tauri::command]
pub async fn recheck_tools(
    state: State<'_, Arc<AppState>>,
    dir: Option<String>,
) -> AppResult<crate::env::Environment> {
    let state = state.inner().clone();
    // Re-check is what a player presses after signing in or out.
    state.forget_github_token();
    off_thread(move || crate::env::environment(&state, dir.as_deref().map(Path::new))).await
}

#[tauri::command]
pub fn tool_instructions(tool: String) -> AppResult<crate::dto::InstallInstructions> {
    crate::env::install_guide(&tool)
}

#[tauri::command]
pub async fn set_git_identity(
    name: String,
    email: String,
    dir: Option<String>,
) -> AppResult<crate::dto::GitIdentity> {
    off_thread(move || {
        let dir = dir.map(PathBuf::from);
        crate::env::set_identity(&name, &email, dir.as_deref())
    })
    .await?
}

#[tauri::command]
pub fn reveal_path(app: tauri::AppHandle, path: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|problem| AppError::new("io", problem.to_string()))
}

#[tauri::command]
pub fn open_url(app: tauri::AppHandle, url: String) -> AppResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(AppError::invalid("only http and https links can be opened"));
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|problem| AppError::new("io", problem.to_string()))
}

#[tauri::command]
pub fn publish_start(
    app: tauri::AppHandle,
    runs: State<'_, Arc<crate::publishing::Runs>>,
    request: crate::publishing::PublishRequest,
) -> AppResult<String> {
    crate::publishing::start(app, Arc::clone(&runs), request)
}

#[tauri::command]
pub fn publish_state(
    runs: State<'_, Arc<crate::publishing::Runs>>,
    run_id: String,
) -> AppResult<crate::publishing::PublishProgress> {
    runs.state(&run_id)
        .ok_or_else(|| AppError::not_found("that publish run is not running"))
}

#[tauri::command]
pub fn publish_cancel(
    runs: State<'_, Arc<crate::publishing::Runs>>,
    run_id: String,
) -> AppResult<()> {
    if runs.cancel(&run_id) {
        Ok(())
    } else {
        Err(AppError::not_found("that publish run is not running"))
    }
}

#[tauri::command]
pub async fn read_index_entry(dir: String) -> AppResult<cartcore::indexentry::IndexEntry> {
    off_thread(move || Ok(cartcore::indexentry::read(&dir_of(&dir)?))).await?
}

#[tauri::command]
pub async fn write_index_entry(
    dir: String,
    entry: cartcore::indexentry::IndexEntry,
) -> AppResult<cartcore::indexentry::IndexEntry> {
    off_thread(move || {
        let path = dir_of(&dir)?;
        let entry = entry.normalized();
        cartcore::indexentry::write(&path, &entry)?;
        Ok(entry)
    })
    .await?
}

/// Write a LICENSE file into the cart directory, so the repository has one.
#[tauri::command]
pub async fn write_license(dir: String, spdx: String, holder: String) -> AppResult<String> {
    off_thread(move || {
        let path = dir_of(&dir)?;
        let text = crate::project::license_text(&spdx, &holder).ok_or_else(|| {
            AppError::invalid(format!("{} is not a licence this app writes", spdx))
        })?;
        std::fs::write(path.join("LICENSE"), &text)?;
        Ok(spdx)
    })
    .await?
}

#[tauri::command]
pub async fn index_readiness(dir: String) -> AppResult<crate::dto::ReadinessReport> {
    off_thread(move || {
        Ok(crate::dto::ReadinessReport::from(
            &crate::publishing::index_readiness(&dir_of(&dir)?)?,
        ))
    })
    .await?
}

#[tauri::command]
pub async fn index_submission_plan(
    state: State<'_, Arc<AppState>>,
    dir: String,
) -> AppResult<crate::dto::SubmissionPlanView> {
    let state = state.inner().clone();
    off_thread(move || {
        Ok(crate::dto::SubmissionPlanView::from(
            &crate::publishing::submission_plan(&state, &dir_of(&dir)?)?,
        ))
    })
    .await?
}

#[tauri::command]
pub async fn index_submit(
    state: State<'_, Arc<AppState>>,
    dir: String,
    edits: crate::publishing::SubmissionEdits,
) -> AppResult<crate::publishing::SubmissionResult> {
    let state = state.inner().clone();
    off_thread(move || crate::publishing::submit(&state, &dir_of(&dir)?, &edits)).await?
}

#[tauri::command]
pub async fn refresh_engine_version(state: State<'_, Arc<AppState>>) -> AppResult<Settings> {
    let state = state.inner().clone();
    off_thread(move || {
        let version = crate::network::latest_engine_version(&state)?;
        state.mutate_settings(|settings| {
            settings.engine_version = version.clone();
            Ok(())
        })
    })
    .await?
}
