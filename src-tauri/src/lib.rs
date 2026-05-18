mod analytics;
mod app;
mod collectors;
mod compat_api;
mod db;
mod models;
mod tray;

use app::commands::{database_healthcheck, get_bootstrap_info, initialize_local_database};
use app::commands::{get_database_summary, list_provider_profiles, save_provider_profile};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_bootstrap_info,
            initialize_local_database,
            database_healthcheck,
            get_database_summary,
            list_provider_profiles,
            save_provider_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
