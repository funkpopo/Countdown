use rusqlite::Connection;

use crate::collectors::claude_code::{
    default_claude_data_dir, ClaudeCodeCollector, CLAUDE_PROVIDER,
};
use crate::collectors::codex::{default_codex_sessions_dir, CodexCollector, CODEX_PROVIDER};
use crate::collectors::managed_launch::run_managed_launch;
use crate::db::repository;
use crate::models::{
    ClaudeCodeSyncSummary, ClaudeOverview, CodexOverview, CodexSyncSummary, ManagedLaunchInput,
    ManagedLaunchResult, RequestRecordUpsertRecord,
};

#[derive(Debug, Clone, Default)]
pub struct CollectorManager;

impl CollectorManager {
    pub fn run_managed_launch(
        connection: &mut Connection,
        input: ManagedLaunchInput,
    ) -> Result<ManagedLaunchResult, String> {
        let capture = run_managed_launch(input)?;

        let provider = capture.request.provider.clone();
        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;

        repository::upsert_session_record(&transaction, &capture.session)?;
        repository::upsert_request_record(&transaction, &capture.request)?;
        repository::rebuild_daily_usage_for_provider(&transaction, &provider)?;
        transaction.commit().map_err(|error| error.to_string())?;

        Ok(capture.result)
    }

    pub fn sync_all_sessions(
        connection: &mut Connection,
    ) -> Result<(CodexSyncSummary, ClaudeCodeSyncSummary), String> {
        // Read the last sync timestamps from app_metadata to enable incremental parsing
        let codex_cutoff = read_last_synced(connection, CODEX_PROVIDER);
        let claude_cutoff = read_last_synced(connection, CLAUDE_PROVIDER);

        let codex_handle =
            std::thread::spawn(move || CodexCollector::import_sessions_since(codex_cutoff));
        let claude_handle =
            std::thread::spawn(move || ClaudeCodeCollector::import_sessions_since(claude_cutoff));

        let codex_import = codex_handle
            .join()
            .map_err(|_| "Codex session import thread panicked".to_string())??;
        let claude_import = claude_handle
            .join()
            .map_err(|_| "Claude Code session import thread panicked".to_string())??;

        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;

        // Track which dates were affected so we can rebuild daily_usage incrementally
        let mut affected_dates: Vec<String> = Vec::new();

        for session in &codex_import.sessions {
            repository::upsert_session_record(&transaction, session)?;
        }
        for request in &codex_import.requests {
            repository::upsert_request_record(&transaction, request)?;
            collect_affected_dates(request, &mut affected_dates);
        }
        if !codex_import.requests.is_empty() {
            repository::rebuild_daily_usage_for_dates(
                &transaction,
                CODEX_PROVIDER,
                &affected_dates,
            )?;
        }

        affected_dates.clear();

        for session in &claude_import.sessions {
            repository::upsert_session_record(&transaction, session)?;
        }
        for request in &claude_import.requests {
            repository::upsert_request_record(&transaction, request)?;
            collect_affected_dates(request, &mut affected_dates);
        }
        if !claude_import.requests.is_empty() {
            repository::rebuild_daily_usage_for_dates(
                &transaction,
                CLAUDE_PROVIDER,
                &affected_dates,
            )?;
        }

        // Write the last-synced timestamp so future incremental syncs can skip unchanged files
        let now_utc = chrono::Utc::now();
        let now_str = now_utc.to_rfc3339();
        repository::set_app_metadata(
            &transaction,
            &format!("last_synced_at_{}", CODEX_PROVIDER),
            &now_str,
        )?;
        repository::set_app_metadata(
            &transaction,
            &format!("last_synced_at_{}", CLAUDE_PROVIDER),
            &now_str,
        )?;

        transaction.commit().map_err(|error| error.to_string())?;

        let (codex_session_count, codex_request_count) =
            repository::get_provider_counts(connection, CODEX_PROVIDER)?;
        let codex_today_usage = repository::get_provider_today_usage(connection, CODEX_PROVIDER)?;
        let (claude_session_count, claude_request_count) =
            repository::get_provider_counts(connection, CLAUDE_PROVIDER)?;
        let claude_today_usage = repository::get_provider_today_usage(connection, CLAUDE_PROVIDER)?;

        Ok((
            CodexSyncSummary {
                data_dir: codex_import.data_dir.display().to_string(),
                data_dir_exists: codex_import.data_dir_exists,
                scanned_files: codex_import.scanned_files,
                imported_sessions: codex_import.sessions.len() as i64,
                imported_requests: codex_import.requests.len() as i64,
                skipped_incomplete_turns: codex_import.skipped_incomplete_turns,
                session_count: codex_session_count,
                request_count: codex_request_count,
                today_usage: codex_today_usage,
            },
            ClaudeCodeSyncSummary {
                data_dir: claude_import.data_dir.display().to_string(),
                data_dir_exists: claude_import.data_dir_exists,
                scanned_files: claude_import.scanned_files,
                imported_sessions: claude_import.sessions.len() as i64,
                imported_requests: claude_import.requests.len() as i64,
                skipped_incomplete_sessions: claude_import.skipped_incomplete_sessions,
                session_count: claude_session_count,
                request_count: claude_request_count,
                today_usage: claude_today_usage,
            },
        ))
    }

    pub fn get_codex_overview(connection: &Connection) -> Result<CodexOverview, String> {
        let data_dir = default_codex_sessions_dir()?;
        let (session_count, request_count) =
            repository::get_provider_counts(connection, CODEX_PROVIDER)?;
        let today_usage = repository::get_provider_today_usage(connection, CODEX_PROVIDER)?;
        let recent_requests =
            repository::list_recent_request_records(connection, CODEX_PROVIDER, 10)?;

        Ok(CodexOverview {
            data_dir: data_dir.display().to_string(),
            data_dir_exists: data_dir.exists(),
            session_count,
            request_count,
            today_usage,
            recent_requests,
        })
    }

    pub fn get_claude_code_overview(connection: &Connection) -> Result<ClaudeOverview, String> {
        let data_dir = default_claude_data_dir()?;
        let (session_count, request_count) =
            repository::get_provider_counts(connection, CLAUDE_PROVIDER)?;
        let today_usage = repository::get_provider_today_usage(connection, CLAUDE_PROVIDER)?;
        let recent_requests =
            repository::list_recent_request_records(connection, CLAUDE_PROVIDER, 10)?;

        Ok(ClaudeOverview {
            data_dir: data_dir.display().to_string(),
            data_dir_exists: data_dir.exists(),
            session_count,
            request_count,
            today_usage,
            recent_requests,
        })
    }
}

/// Extract the date portion from a request's started_at or finished_at and add it to
/// the affected_dates set (deduplicated).
fn collect_affected_dates(request: &RequestRecordUpsertRecord, dates: &mut Vec<String>) {
    let source = request
        .finished_at
        .as_deref()
        .unwrap_or(&request.started_at);
    // Extract YYYY-MM-DD from an ISO timestamp or date string
    if source.len() >= 10 {
        let date = &source[..10]; // "2024-01-15" portion
        if !dates.iter().any(|d| d == date) {
            dates.push(date.to_string());
        }
    }
}

/// Read the last_synced_at_{provider} timestamp from app_metadata.
/// Returns None when there is no prior sync (first run), which causes the
/// collector to parse all files.
fn read_last_synced(
    connection: &Connection,
    provider: &str,
) -> Option<chrono::DateTime<chrono::Utc>> {
    let key = format!("last_synced_at_{}", provider);
    match repository::get_app_metadata(connection, &key) {
        Ok(Some(value)) => chrono::DateTime::parse_from_rfc3339(&value)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        _ => None,
    }
}
