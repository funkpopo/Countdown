use rusqlite::Connection;

use crate::collectors::claude_code::{
    default_claude_data_dir, ClaudeCodeCollector, CLAUDE_PROVIDER,
};
use crate::collectors::codex::{default_codex_sessions_dir, CodexCollector, CODEX_PROVIDER};
use crate::db::repository;
use crate::models::{ClaudeCodeSyncSummary, ClaudeOverview, CodexOverview, CodexSyncSummary};

#[derive(Debug, Clone, Default)]
pub struct CollectorManager;

impl CollectorManager {
    pub fn sync_codex_sessions(connection: &mut Connection) -> Result<CodexSyncSummary, String> {
        let import = CodexCollector::import_default_sessions()?;

        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;

        for session in &import.sessions {
            repository::upsert_session_record(&transaction, session)?;
        }

        for request in &import.requests {
            repository::upsert_request_record(&transaction, request)?;
        }

        repository::rebuild_daily_usage_for_provider(&transaction, CODEX_PROVIDER)?;
        transaction.commit().map_err(|error| error.to_string())?;

        let (session_count, request_count) =
            repository::get_provider_counts(connection, CODEX_PROVIDER)?;
        let today_usage = repository::get_provider_today_usage(connection, CODEX_PROVIDER)?;

        Ok(CodexSyncSummary {
            data_dir: import.data_dir.display().to_string(),
            data_dir_exists: import.data_dir_exists,
            scanned_files: import.scanned_files,
            imported_sessions: import.sessions.len() as i64,
            imported_requests: import.requests.len() as i64,
            skipped_incomplete_turns: import.skipped_incomplete_turns,
            session_count,
            request_count,
            today_usage,
        })
    }

    pub fn get_codex_overview(connection: &Connection) -> Result<CodexOverview, String> {
        let data_dir = default_codex_sessions_dir()?;
        let (session_count, request_count) =
            repository::get_provider_counts(connection, CODEX_PROVIDER)?;
        let today_usage = repository::get_provider_today_usage(connection, CODEX_PROVIDER)?;
        let recent_requests =
            repository::list_recent_request_records(connection, CODEX_PROVIDER, 12)?;

        Ok(CodexOverview {
            data_dir: data_dir.display().to_string(),
            data_dir_exists: data_dir.exists(),
            session_count,
            request_count,
            today_usage,
            recent_requests,
        })
    }

    pub fn sync_claude_code_sessions(
        connection: &mut Connection,
    ) -> Result<ClaudeCodeSyncSummary, String> {
        let import = ClaudeCodeCollector::import_default_sessions()?;

        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;

        for session in &import.sessions {
            repository::upsert_session_record(&transaction, session)?;
        }

        for request in &import.requests {
            repository::upsert_request_record(&transaction, request)?;
        }

        repository::rebuild_daily_usage_for_provider(&transaction, CLAUDE_PROVIDER)?;
        transaction.commit().map_err(|error| error.to_string())?;

        let (session_count, request_count) =
            repository::get_provider_counts(connection, CLAUDE_PROVIDER)?;
        let today_usage = repository::get_provider_today_usage(connection, CLAUDE_PROVIDER)?;

        Ok(ClaudeCodeSyncSummary {
            data_dir: import.data_dir.display().to_string(),
            data_dir_exists: import.data_dir_exists,
            scanned_files: import.scanned_files,
            imported_sessions: import.sessions.len() as i64,
            imported_requests: import.requests.len() as i64,
            skipped_incomplete_sessions: import.skipped_incomplete_sessions,
            session_count,
            request_count,
            today_usage,
        })
    }

    pub fn get_claude_code_overview(connection: &Connection) -> Result<ClaudeOverview, String> {
        let data_dir = default_claude_data_dir()?;
        let (session_count, request_count) =
            repository::get_provider_counts(connection, CLAUDE_PROVIDER)?;
        let today_usage = repository::get_provider_today_usage(connection, CLAUDE_PROVIDER)?;
        let recent_requests =
            repository::list_recent_request_records(connection, CLAUDE_PROVIDER, 12)?;

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
