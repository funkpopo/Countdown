use std::time::Instant;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::compat_api;
use crate::db;
use crate::db::repository;
use crate::localization;
use crate::models::{
    BootstrapInfo, ClaudeOverview, CodexOverview, CombinedTodayUsage, CombinedUsage,
    CompatApiStatus, DatabaseHealth, DatabaseSummary, DateRangeInput, ManagedLaunchInput,
    ManagedLaunchResult, PaginatedRequestRecords, PerformanceQualitySummary,
    ProviderHealthCheckResult, ProviderProfileRecord, ProviderProfileUpsertInput,
    ProviderRuntimeStatus, QuickViewSummary, RequestFilterInput, RequestFilterOptions,
    RequestRecordDetail, UsageHistogram, UsageHistogramInput,
};

const UI_LANGUAGE_CHANGED_EVENT: &str = "ui-language-changed";
const COMPAT_API_STATUS_CHANGED_EVENT: &str = "compat-api-status-changed";
const WIZARD_COMPLETED_KEY: &str = "wizard_completed";

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
pub fn is_first_launch(app: AppHandle) -> Result<bool, String> {
    if let Ok(connection) = db::get_connection(&app) {
        if let Ok(Some(value)) = repository::get_app_metadata(&connection, WIZARD_COMPLETED_KEY) {
            return Ok(value != "true");
        }
    }
    Ok(true)
}

#[tauri::command]
pub fn complete_wizard(app: AppHandle) -> Result<(), String> {
    db::initialize(&app)?;
    let connection = db::get_connection(&app)?;
    repository::set_app_metadata(&connection, WIZARD_COMPLETED_KEY, "true")?;
    Ok(())
}

#[tauri::command]
pub fn is_db_initialized(app: AppHandle) -> Result<bool, String> {
    let database_path = db::database_path(&app)?;
    Ok(database_path.exists())
}

#[tauri::command]
pub fn get_ui_language(app: AppHandle) -> Result<String, String> {
    Ok(localization::resolve_ui_language(&app).as_str().to_string())
}

#[tauri::command]
pub async fn set_ui_language(app: AppHandle, language: String) -> Result<String, String> {
    let language = localization::persist_ui_language(&app, &language)?;
    let value = language.as_str().to_string();
    let _ = app.emit(
        UI_LANGUAGE_CHANGED_EVENT,
        json!({ "language": value.clone() }),
    );
    let _ = crate::tray::refresh_tray_menu(&app).await;

    Ok(value)
}

#[tauri::command]
pub fn open_main_page(app: AppHandle, page: String, period: Option<String>) -> Result<(), String> {
    let page = normalize_main_page(&page)?;
    let period = period
        .as_deref()
        .map(normalize_overview_period)
        .transpose()?;

    if let Some(tray_runtime) = app.try_state::<crate::tray::TrayRuntime>() {
        let _ = tray_runtime.cancel_hover_for_app(&app);
    }

    crate::tray::show_main_window_page(&app, page, period)
}

#[tauri::command]
pub fn quick_view_pointer_enter(app: AppHandle) -> Result<(), String> {
    if let Some(tray_runtime) = app.try_state::<crate::tray::TrayRuntime>() {
        tray_runtime.cancel_pending_hover_action();
    }
    Ok(())
}

#[tauri::command]
pub fn quick_view_pointer_leave(app: AppHandle) -> Result<(), String> {
    if let Some(tray_runtime) = app.try_state::<crate::tray::TrayRuntime>() {
        tray_runtime.cancel_hover_for_app(&app)?;
    }
    Ok(())
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
pub fn delete_provider_profile(app: AppHandle, id: String) -> Result<(), String> {
    db::initialize(&app)?;
    db::delete_provider_profile(&app, id)
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
pub async fn get_quick_view_summary(app: AppHandle) -> Result<QuickViewSummary, String> {
    db::initialize(&app)?;
    let usage = db::combined_today_usage(&app)?;
    let (recent_one_hour_request_count, recent_one_hour_error_count) =
        db::recent_request_window_summary(&app, "-1 hour")?;
    let compat_api = compat_api::get_status(&app).await;
    let recent_one_hour_error_rate = if recent_one_hour_request_count == 0 {
        0.0
    } else {
        recent_one_hour_error_count as f64 / recent_one_hour_request_count as f64
    };

    Ok(QuickViewSummary {
        compat_api_running: compat_api.running,
        compat_api_listen_address: compat_api.listen_address,
        compat_api_started_at: compat_api.started_at,
        compat_api_profiles_count: compat_api.profiles_count,
        recent_one_hour_request_count,
        recent_one_hour_error_count,
        recent_one_hour_error_rate,
        usage,
    })
}

#[tauri::command]
pub fn get_combined_usage(app: AppHandle, range: DateRangeInput) -> Result<CombinedUsage, String> {
    db::initialize(&app)?;
    db::combined_usage_for_range(&app, range.start_date, range.end_date)
}

#[tauri::command]
pub fn get_combined_usage_total(app: AppHandle) -> Result<CombinedUsage, String> {
    db::initialize(&app)?;
    db::combined_usage_total(&app)
}

#[tauri::command]
pub fn get_usage_histogram(
    app: AppHandle,
    input: UsageHistogramInput,
) -> Result<UsageHistogram, String> {
    db::initialize(&app)?;
    db::usage_histogram(&app, input.period, input.granularity)
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
pub fn get_request_filter_options(app: AppHandle) -> Result<RequestFilterOptions, String> {
    db::initialize(&app)?;
    db::request_filter_options(&app)
}

#[tauri::command]
pub fn get_provider_runtime_statuses(app: AppHandle) -> Result<Vec<ProviderRuntimeStatus>, String> {
    db::initialize(&app)?;
    db::provider_runtime_statuses(&app)
}

#[tauri::command]
pub fn get_performance_quality_summary(
    app: AppHandle,
) -> Result<PerformanceQualitySummary, String> {
    db::initialize(&app)?;
    db::performance_quality_summary(&app)
}

#[tauri::command]
pub async fn check_provider_health(
    app: AppHandle,
    provider_id: String,
) -> Result<ProviderHealthCheckResult, String> {
    db::initialize(&app)?;
    let profiles = db::list_provider_profiles(&app)?;
    let profile = profiles
        .into_iter()
        .find(|profile| profile.id == provider_id)
        .ok_or_else(|| format!("Provider profile not found: {provider_id}"))?;
    run_provider_health_check(profile).await
}

#[tauri::command]
pub async fn check_all_provider_health(
    app: AppHandle,
) -> Result<Vec<ProviderHealthCheckResult>, String> {
    db::initialize(&app)?;
    let profiles = db::list_provider_profiles(&app)?;
    let mut results = Vec::with_capacity(profiles.len());

    for profile in profiles {
        results.push(run_provider_health_check(profile).await?);
    }

    Ok(results)
}

async fn run_provider_health_check(
    profile: ProviderProfileRecord,
) -> Result<ProviderHealthCheckResult, String> {
    let checked_at = chrono::Utc::now().to_rfc3339();
    let endpoint = provider_health_endpoint(&profile)?;
    let client = reqwest::Client::new();
    let mut request = client.get(endpoint);

    if let Some(api_key_env) = profile.api_key_env.as_deref() {
        match std::env::var(api_key_env) {
            Ok(api_key) if !api_key.trim().is_empty() => {
                if profile.api_format == "anthropic" {
                    request = request
                        .header("x-api-key", api_key)
                        .header("anthropic-version", "2023-06-01");
                } else {
                    request = request.header("Authorization", format!("Bearer {api_key}"));
                }
            }
            _ => {
                return Ok(ProviderHealthCheckResult {
                    provider_key: profile.provider_key,
                    display_name: profile.display_name,
                    checked_at,
                    available: false,
                    status_code: None,
                    latency_ms: None,
                    error_text: Some(format!(
                        "Missing API key environment variable: {api_key_env}"
                    )),
                });
            }
        }
    }

    let started = Instant::now();
    let response = request.send().await;
    let latency_ms = started.elapsed().as_millis() as i64;

    match response {
        Ok(response) => {
            let status = response.status();
            let error_text = if status.is_success() {
                None
            } else {
                Some(response.text().await.unwrap_or_default())
            };

            Ok(ProviderHealthCheckResult {
                provider_key: profile.provider_key,
                display_name: profile.display_name,
                checked_at,
                available: status.is_success(),
                status_code: Some(status.as_u16()),
                latency_ms: Some(latency_ms),
                error_text,
            })
        }
        Err(error) => Ok(ProviderHealthCheckResult {
            provider_key: profile.provider_key,
            display_name: profile.display_name,
            checked_at,
            available: false,
            status_code: None,
            latency_ms: Some(latency_ms),
            error_text: Some(error.to_string()),
        }),
    }
}

fn provider_health_endpoint(profile: &ProviderProfileRecord) -> Result<String, String> {
    let base_url = profile
        .base_url
        .as_deref()
        .or_else(|| match profile.api_format.as_str() {
            "openai" => Some("https://api.openai.com"),
            "anthropic" => Some("https://api.anthropic.com"),
            _ => None,
        })
        .ok_or_else(|| format!("Provider {} has no base URL", profile.display_name))?;

    Ok(format!("{}/v1/models", base_url.trim_end_matches('/')))
}

fn normalize_main_page(page: &str) -> Result<&'static str, String> {
    match page {
        "overview" => Ok("overview"),
        "requests" => Ok("requests"),
        "settings" => Ok("settings"),
        _ => Err(format!("Unknown main window page: {page}")),
    }
}

fn normalize_overview_period(period: &str) -> Result<&'static str, String> {
    match period {
        "today" => Ok("today"),
        "week" => Ok("week"),
        "month" => Ok("month"),
        "total" => Ok("total"),
        _ => Err(format!("Unknown overview period: {period}")),
    }
}

#[tauri::command]
pub async fn start_compat_api_server(
    app: AppHandle,
    listen_address: String,
) -> Result<CompatApiStatus, String> {
    let status = compat_api::start_server(&app, listen_address).await?;
    emit_compat_api_status_changed(&app, &status).await;
    Ok(status)
}

#[tauri::command]
pub async fn stop_compat_api_server(app: AppHandle) -> Result<CompatApiStatus, String> {
    let status = compat_api::stop_server(&app).await?;
    emit_compat_api_status_changed(&app, &status).await;
    Ok(status)
}

#[tauri::command]
pub async fn get_compat_api_status(app: AppHandle) -> Result<CompatApiStatus, String> {
    Ok(compat_api::get_status(&app).await)
}

async fn emit_compat_api_status_changed(app: &AppHandle, status: &CompatApiStatus) {
    let _ = app.emit(COMPAT_API_STATUS_CHANGED_EVENT, status.clone());
    let _ = crate::tray::refresh_tray_menu(app).await;
}
