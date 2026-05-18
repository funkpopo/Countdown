use tauri::{AppHandle, Manager};

use crate::db;
use crate::models::{BootstrapInfo, DatabaseHealth};

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
