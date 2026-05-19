mod migrations;
pub(crate) mod repository;

use std::fs;
use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension};
use tauri::{path::BaseDirectory, AppHandle, Manager};

use crate::collectors::manager::CollectorManager;
use crate::models::{
    ClaudeCodeSyncSummary, ClaudeOverview, CodexOverview, CodexSyncSummary, CombinedTodayUsage,
    DatabaseHealth, DatabaseSummary, ManagedLaunchInput, ManagedLaunchResult,
    PaginatedRequestRecords, ProviderProfileRecord, ProviderProfileUpsertInput, RequestFilterInput,
    RequestRecordDetail,
};

const DATABASE_FILE: &str = "countdown.db";

pub fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve(DATABASE_FILE, BaseDirectory::AppData)
        .map_err(|error| error.to_string())
}

pub fn initialize(app: &AppHandle) -> Result<(), String> {
    let connection = open_connection(app)?;
    migrations::apply_migrations(&connection)
}

pub fn healthcheck(app: &AppHandle) -> Result<DatabaseHealth, String> {
    let database_path = database_path(app)?;
    let exists = database_path.exists();
    let writable = database_path
        .parent()
        .map(|parent| parent.exists())
        .unwrap_or(false);

    if !exists {
        return Ok(DatabaseHealth {
            database_path: database_path.display().to_string(),
            exists,
            writable,
            schema_version: None,
            initialized_at: None,
            migration_count: 0,
        });
    }

    let connection = open_connection(app)?;
    let schema_version = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let initialized_at = connection
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'initialized_at'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let migration_count = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0);

    Ok(DatabaseHealth {
        database_path: database_path.display().to_string(),
        exists,
        writable,
        schema_version,
        initialized_at,
        migration_count,
    })
}

pub fn database_summary(app: &AppHandle) -> Result<DatabaseSummary, String> {
    let connection = open_connection(app)?;
    repository::get_database_summary(&connection)
}

pub fn list_provider_profiles(app: &AppHandle) -> Result<Vec<ProviderProfileRecord>, String> {
    let connection = open_connection(app)?;
    repository::list_provider_profiles(&connection)
}

pub fn list_provider_profiles_from_conn(
    connection: &Connection,
) -> Result<Vec<ProviderProfileRecord>, String> {
    repository::list_provider_profiles(connection)
}

pub fn save_provider_profile(
    app: &AppHandle,
    input: ProviderProfileUpsertInput,
) -> Result<ProviderProfileRecord, String> {
    let connection = open_connection(app)?;
    repository::upsert_provider_profile(&connection, &input)
}

pub fn save_provider_profiles_batch(
    app: &AppHandle,
    inputs: Vec<ProviderProfileUpsertInput>,
) -> Result<Vec<ProviderProfileRecord>, String> {
    let mut connection = open_connection(app)?;
    repository::upsert_provider_profiles(&mut connection, &inputs)
}

pub fn sync_codex_sessions(app: &AppHandle) -> Result<CodexSyncSummary, String> {
    let mut connection = open_connection(app)?;
    CollectorManager::sync_codex_sessions(&mut connection)
}

pub fn run_managed_launch(
    app: &AppHandle,
    input: ManagedLaunchInput,
) -> Result<ManagedLaunchResult, String> {
    let mut connection = open_connection(app)?;
    CollectorManager::run_managed_launch(&mut connection, input)
}

pub fn codex_overview(app: &AppHandle) -> Result<CodexOverview, String> {
    let connection = open_connection(app)?;
    CollectorManager::get_codex_overview(&connection)
}

pub fn sync_claude_code_sessions(app: &AppHandle) -> Result<ClaudeCodeSyncSummary, String> {
    let mut connection = open_connection(app)?;
    CollectorManager::sync_claude_code_sessions(&mut connection)
}

pub fn claude_code_overview(app: &AppHandle) -> Result<ClaudeOverview, String> {
    let connection = open_connection(app)?;
    CollectorManager::get_claude_code_overview(&connection)
}

pub fn combined_today_usage(app: &AppHandle) -> Result<CombinedTodayUsage, String> {
    let connection = open_connection(app)?;
    repository::get_combined_today_usage(&connection)
}

pub fn list_filtered_requests(
    app: &AppHandle,
    filter: RequestFilterInput,
) -> Result<PaginatedRequestRecords, String> {
    let connection = open_connection(app)?;
    repository::list_filtered_request_records(&connection, &filter)
}

pub fn get_request_detail(app: &AppHandle, id: String) -> Result<RequestRecordDetail, String> {
    let connection = open_connection(app)?;
    repository::get_request_record_detail(&connection, &id)
}

pub(crate) fn open_connection(app: &AppHandle) -> Result<Connection, String> {
    let database_path = database_path(app)?;

    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let connection = Connection::open(database_path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            ",
        )
        .map_err(|error| error.to_string())?;

    Ok(connection)
}

pub fn get_connection(app: &AppHandle) -> Result<Connection, String> {
    open_connection(app)
}

pub fn upsert_request_record(
    connection: &Connection,
    record: &crate::models::RequestRecordUpsertRecord,
) -> Result<(), String> {
    repository::upsert_request_record(connection, record)
}

pub fn rebuild_daily_usage_for_provider(
    connection: &Connection,
    provider: &str,
) -> Result<(), String> {
    repository::rebuild_daily_usage_for_provider(connection, provider)
}
