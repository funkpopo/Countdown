use tauri::{AppHandle, Manager};

use crate::db;
use crate::models::{
    BootstrapInfo, CodexOverview, CodexSyncSummary, DatabaseHealth, DatabaseSummary,
    ProviderProfileRecord, ProviderProfileUpsertInput,
};

#[tauri::command]
pub fn get_bootstrap_info(app: AppHandle) -> Result<BootstrapInfo, String> {
    let package_info = app.package_info();
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let database_path = db::database_path(&app)?;

    Ok(BootstrapInfo {
        product_name: package_info.name.clone(),
        version: package_info.version.to_string(),
        identifier: app.config().identifier.clone(),
        app_data_dir: app_data_dir.display().to_string(),
        database_path: database_path.display().to_string(),
        phase0_complete: true,
        phase1_complete: true,
        phase2_complete: true,
        phase3_complete: false,
    })
}

#[tauri::command]
pub fn initialize_local_database(app: AppHandle) -> Result<DatabaseHealth, String> {
    db::initialize(&app)?;
    db::healthcheck(&app)
}

#[tauri::command]
pub fn database_healthcheck(app: AppHandle) -> Result<DatabaseHealth, String> {
    db::healthcheck(&app)
}

#[tauri::command]
pub fn get_database_summary(app: AppHandle) -> Result<DatabaseSummary, String> {
    db::initialize(&app)?;
    db::database_summary(&app)
}

#[tauri::command]
pub fn list_provider_profiles(app: AppHandle) -> Result<Vec<ProviderProfileRecord>, String> {
    db::initialize(&app)?;
    db::list_provider_profiles(&app)
}

#[tauri::command]
pub fn save_provider_profile(
    app: AppHandle,
    input: ProviderProfileUpsertInput,
) -> Result<ProviderProfileRecord, String> {
    db::initialize(&app)?;
    db::save_provider_profile(&app, input)
}

#[tauri::command]
pub fn sync_codex_sessions(app: AppHandle) -> Result<CodexSyncSummary, String> {
    db::initialize(&app)?;
    db::sync_codex_sessions(&app)
}

#[tauri::command]
pub fn get_codex_overview(app: AppHandle) -> Result<CodexOverview, String> {
    db::initialize(&app)?;
    db::codex_overview(&app)
}
