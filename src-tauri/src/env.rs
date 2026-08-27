//! Environment detection and the install guidance a missing tool needs. The app
//! never runs a package manager: it shows the command and offers a re-check.

use crate::dto::{GhTool, GitIdentity, InstallInstructions, Tool as ToolView};
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use crate::state::AppState;
use serde::Serialize;
use std::path::Path;
use toolchain::detect::{self, TokenEnv};
use toolchain::instructions::{self, Tool};
use toolchain::runner::{CancelToken, SystemRunner};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub os: String,
    pub arch: String,
    pub app_version: String,
    pub engine_version: String,
    pub mod_api: u32,
    pub paths: AppPaths,
    pub git: ToolView,
    pub gh: GhTool,
    pub identity: GitIdentity,
    pub platform: String,
}

pub fn environment(state: &AppState, dir: Option<&Path>) -> Environment {
    let runner = SystemRunner;
    let cancel = CancelToken::new();
    let tools = detect::detect(&runner, &cancel);
    let token_env = TokenEnv::from_env();
    let auth = if tools.gh.found {
        detect::gh_auth_status(&runner, &cancel, token_env).unwrap_or_else(|problem| {
            detect::parse_auth_status(&problem.to_string(), false, token_env)
        })
    } else {
        detect::parse_auth_status("", false, token_env)
    };
    let identity = match dir {
        Some(dir) if dir.is_dir() => detect::dir_identity(&runner, &cancel, dir),
        _ => detect::global_identity(&runner, &cancel),
    };
    let settings = state.settings();
    Environment {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        engine_version: settings.engine_version,
        mod_api: settings.mod_api,
        paths: state.paths.clone(),
        git: ToolView::from(&tools.git),
        gh: GhTool::new(&tools.gh, &auth),
        identity: GitIdentity::from(&identity),
        platform: format!("{:?}", instructions::detect_platform()).to_lowercase(),
    }
}

pub fn install_guide(tool: &str) -> AppResult<InstallInstructions> {
    let tool = match tool {
        "git" => Tool::Git,
        "gh" => Tool::Gh,
        other => return Err(AppError::invalid(format!("unknown tool {}", other))),
    };
    Ok(InstallInstructions::from(&instructions::guide(
        tool,
        instructions::detect_platform(),
    )))
}

/// Writes the LOCAL config of a cart directory; the app never edits global git.
pub fn set_identity(name: &str, email: &str, dir: Option<&Path>) -> AppResult<GitIdentity> {
    let dir = dir.filter(|dir| dir.is_dir()).ok_or_else(|| {
        AppError::invalid("open a cart first; the identity is written to that repository")
    })?;
    let runner = SystemRunner;
    let cancel = CancelToken::new();
    detect::set_identity(&runner, &cancel, dir, name, email)
        .map_err(|problem| AppError::new("git", problem.to_string()))?;
    Ok(GitIdentity::from(&detect::dir_identity(
        &runner, &cancel, dir,
    )))
}
