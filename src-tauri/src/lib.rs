mod analytics;
mod app;
mod collectors;
mod compat_api;
mod db;
mod models;
mod tray;

use app::commands::{database_healthcheck, get_bootstrap_info, initialize_local_database};
use app::commands::{get_claude_code_overview, sync_claude_code_sessions};
use app::commands::{get_codex_overview, sync_codex_sessions};
use app::commands::{get_combined_today_usage, get_database_summary};
use app::commands::{get_compat_api_status, start_compat_api_server, stop_compat_api_server};
use app::commands::{get_request_detail, list_filtered_requests};
use app::commands::{list_provider_profiles, save_provider_profile, save_provider_profiles_batch};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let tray_runtime = tray::TrayRuntime::new();
    let tray_runtime_for_setup = tray_runtime.clone();
    let tray_runtime_for_events = tray_runtime.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            tray_runtime_for_setup.setup(app.handle())?;
            Ok(())
        })
        .on_window_event(move |window, event| {
            tray_runtime_for_events.handle_window_event(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            get_bootstrap_info,
            initialize_local_database,
            database_healthcheck,
            get_database_summary,
            list_provider_profiles,
            save_provider_profile,
            save_provider_profiles_batch,
            sync_codex_sessions,
            get_codex_overview,
            sync_claude_code_sessions,
            get_claude_code_overview,
            get_combined_today_usage,
            list_filtered_requests,
            get_request_detail,
            start_compat_api_server,
            stop_compat_api_server,
            get_compat_api_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
