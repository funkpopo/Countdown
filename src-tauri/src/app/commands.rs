use std::time::Instant;

use tauri::{AppHandle, Manager};

use crate::compat_api::CompatApiServer;
use crate::db;
use crate::models::{
    BootstrapInfo, ClaudeOverview, CodexOverview, CombinedTodayUsage, CombinedUsage,
    CompatApiStatus, DatabaseHealth, DatabaseSummary, DateRangeInput, ManagedLaunchInput,
    ManagedLaunchResult, PaginatedRequestRecords, PerformanceQualitySummary,
    ProviderHealthCheckResult, ProviderProfileRecord, ProviderProfileUpsertInput,
    ProviderRuntimeStatus, RequestFilterInput, RequestFilterOptions, RequestRecordDetail,
    UsageHistogram, UsageHistogramInput,
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
pub fn get_performance_quality_summary(app: AppHandle) -> Result<PerformanceQualitySummary, String> {
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

#[tauri::command]
pub async fn start_compat_api_server(
    app: AppHandle,
    listen_address: String,
) -> Result<CompatApiStatus, String> {
    if let Some(server) = app.try_state::<CompatApiServer>() {
        if server.get_status().await.running {
            return Ok(server.get_status().await);
        }

        server.set_listen_address(listen_address).await?;
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
