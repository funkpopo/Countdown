use tauri::{AppHandle, Manager};

use crate::compat_api::CompatApiServer;
use crate::db;
use crate::models::{
    BootstrapInfo, ClaudeOverview, CodexOverview, CombinedTodayUsage, CombinedUsage, CompatApiStatus,
    DatabaseHealth, DatabaseSummary, DateRangeInput, ManagedLaunchInput, ManagedLaunchResult,
    PaginatedRequestRecords, ProviderProfileRecord, ProviderProfileUpsertInput, RequestFilterInput,
    RequestRecordDetail,
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
        phase3_complete: true,
        phase4_complete: true,
        phase5_complete: true,
        phase6_complete: true,
        phase7_complete: true,
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
pub fn save_provider_profiles_batch(
    app: AppHandle,
    inputs: Vec<ProviderProfileUpsertInput>,
) -> Result<Vec<ProviderProfileRecord>, String> {
    db::initialize(&app)?;
    db::save_provider_profiles_batch(&app, inputs)
}

#[tauri::command]
pub fn get_codex_overview(app: AppHandle) -> Result<CodexOverview, String> {
    db::initialize(&app)?;
    db::codex_overview(&app)
}

#[tauri::command]
pub async fn run_managed_launch(
    app: AppHandle,
    input: ManagedLaunchInput,
) -> Result<ManagedLaunchResult, String> {
    let app_clone = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        db::initialize(&app_clone)?;
        db::run_managed_launch(&app_clone, input)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn get_claude_code_overview(app: AppHandle) -> Result<ClaudeOverview, String> {
    db::initialize(&app)?;
    db::claude_code_overview(&app)
}

#[tauri::command]
pub fn get_combined_today_usage(app: AppHandle) -> Result<CombinedTodayUsage, String> {
    db::initialize(&app)?;
    db::combined_today_usage(&app)
}

#[tauri::command]
pub fn get_combined_usage(
    app: AppHandle,
    range: DateRangeInput,
) -> Result<CombinedUsage, String> {
    db::initialize(&app)?;
    db::combined_usage_for_range(&app, range.start_date, range.end_date)
}

#[tauri::command]
pub fn list_filtered_requests(
    app: AppHandle,
    filter: RequestFilterInput,
) -> Result<PaginatedRequestRecords, String> {
    db::initialize(&app)?;
    db::list_filtered_requests(&app, filter)
}

#[tauri::command]
pub fn get_request_detail(app: AppHandle, id: String) -> Result<RequestRecordDetail, String> {
    db::initialize(&app)?;
    db::get_request_detail(&app, id)
}

#[tauri::command]
pub async fn start_compat_api_server(
    app: AppHandle,
    listen_address: String,
) -> Result<CompatApiStatus, String> {
    if let Some(server) = app.try_state::<CompatApiServer>() {
        if server.get_status().await.running {
            return Ok(server.get_status().await);
        }

        server.start().await?;
        return Ok(server.get_status().await);
    }

    let server = CompatApiServer::new(app.clone(), listen_address);
    app.manage(server);

    let server = app.state::<CompatApiServer>();
    server.start().await?;
    Ok(server.get_status().await)
}

#[tauri::command]
pub async fn stop_compat_api_server(app: AppHandle) -> Result<CompatApiStatus, String> {
    let server = app.state::<CompatApiServer>();
    server.stop().await?;
    Ok(server.get_status().await)
}

#[tauri::command]
pub async fn get_compat_api_status(app: AppHandle) -> Result<CompatApiStatus, String> {
    match app.try_state::<CompatApiServer>() {
        Some(server) => Ok(server.get_status().await),
        None => Ok(CompatApiStatus {
            running: false,
            listen_address: "127.0.0.1:8688".to_string(),
            started_at: None,
            profiles_count: 0,
        }),
    }
}
