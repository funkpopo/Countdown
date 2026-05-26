use rusqlite::Connection;

use crate::collectors::claude_code::{
    default_claude_data_dir, ClaudeCodeCollector, CLAUDE_PROVIDER,
};
use crate::collectors::codex::{default_codex_sessions_dir, CodexCollector, CODEX_PROVIDER};
use crate::collectors::managed_launch::run_managed_launch;
use crate::db::repository;
use crate::models::{
    ClaudeCodeSyncSummary, ClaudeOverview, CodexOverview, CodexSyncSummary, ManagedLaunchInput,
    ManagedLaunchResult,
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
        let codex_handle = std::thread::spawn(CodexCollector::import_default_sessions);
        let claude_handle = std::thread::spawn(ClaudeCodeCollector::import_default_sessions);

        let codex_import = codex_handle
            .join()
            .map_err(|_| "Codex session import thread panicked".to_string())??;
        let claude_import = claude_handle
            .join()
            .map_err(|_| "Claude Code session import thread panicked".to_string())??;

        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;

        for session in &codex_import.sessions {
            repository::upsert_session_record(&transaction, session)?;
        }
        for request in &codex_import.requests {
            repository::upsert_request_record(&transaction, request)?;
        }
        repository::rebuild_daily_usage_for_provider(&transaction, CODEX_PROVIDER)?;

        for session in &claude_import.sessions {
            repository::upsert_session_record(&transaction, session)?;
        }
        for request in &claude_import.requests {
            repository::upsert_request_record(&transaction, request)?;
        }
        repository::rebuild_daily_usage_for_provider(&transaction, CLAUDE_PROVIDER)?;

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
