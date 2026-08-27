//! Tauri host for the cart maker. All real work lives in the library crates.

pub mod commands;
pub mod dto;
pub mod env;
pub mod error;
pub mod label;
pub mod network;
pub mod options;
pub mod paths;
pub mod project;
pub mod publishing;
pub mod settings;
pub mod state;

use state::AppState;

pub fn run() {
    let app_state = AppState::load().expect("app data directories must be writable");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(std::sync::Arc::new(app_state))
        .manage(std::sync::Arc::new(publishing::Runs::default()))
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::cache_usage,
            commands::clear_cache,
            commands::export_app_data,
            commands::forget_project,
            commands::scaffold_project,
            commands::open_project,
            commands::save_project,
            commands::validate_project,
            commands::bundle_name,
            commands::export_bundle,
            commands::write_workflow,
            commands::add_pin,
            commands::remove_pin,
            commands::reorder_pins,
            commands::set_pin_options,
            commands::set_pin_enabled,
            commands::label_templates,
            commands::read_label_doc,
            commands::write_label_doc,
            commands::check_label_export,
            commands::write_label_png,
            commands::placeholder_label,
            commands::read_image_data_url,
            commands::mod_options_from_install,
            commands::index_sources,
            commands::add_index_source,
            commands::remove_index_source,
            commands::resolve_spec,
            commands::github_releases,
            commands::gamebanana_files,
            commands::validate_online,
            commands::fetch_index,
            commands::fetch_thumbnail,
            commands::mod_options_from_archive,
            commands::app_environment,
            commands::recheck_tools,
            commands::tool_instructions,
            commands::set_git_identity,
            commands::reveal_path,
            commands::open_url,
            commands::publish_start,
            commands::publish_state,
            commands::publish_cancel,
            commands::read_index_entry,
            commands::write_index_entry,
            commands::write_license,
            commands::index_readiness,
            commands::index_submission_plan,
            commands::index_submit,
            commands::refresh_engine_version,
        ])
        .run(tauri::generate_context!())
        .expect("the cart maker window could not start");
}
