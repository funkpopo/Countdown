use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::models::{RequestRecordUpsertRecord, RequestType, SessionUpsertRecord};

pub const CODEX_PROVIDER: &str = "codex";
pub const PASSIVE_SOURCE_MODE: &str = "passive_ingest";

#[derive(Debug, Clone, Default)]
pub struct CodexCollector;

#[derive(Debug, Clone)]
pub struct CodexImportResult {
    pub data_dir: PathBuf,
    pub data_dir_exists: bool,
    pub scanned_files: i64,
    pub sessions: Vec<SessionUpsertRecord>,
    pub requests: Vec<RequestRecordUpsertRecord>,
    pub skipped_incomplete_turns: i64,
}

#[derive(Debug, Clone)]
struct SessionMeta {
    id: String,
    timestamp: String,
    cwd: Option<String>,
    originator: Option<String>,
    cli_version: Option<String>,
    source: Option<String>,
    thread_source: Option<String>,
    model_provider: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct TokenUsage {
    input_tokens: i64,
    cached_input_tokens: i64,
    output_tokens: i64,
    reasoning_tokens: i64,
}

#[derive(Debug, Clone, Default)]
struct TurnState {
    turn_id: String,
    cwd: Option<String>,
    model: Option<String>,
    timezone: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    ttft_ms: Option<i64>,
    duration_ms: Option<i64>,
    token_usage: Option<TokenUsage>,
    token_updates: i64,
    last_agent_message: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedRollout {
    session: Option<SessionUpsertRecord>,
    requests: Vec<RequestRecordUpsertRecord>,
}

impl CodexCollector {
    pub fn import_default_sessions() -> Result<CodexImportResult, String> {
        Self::import_sessions_since(None)
    }

    pub fn import_sessions_since(
        after_time: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<CodexImportResult, String> {
        let data_dir = default_codex_sessions_dir()?;
        if !data_dir.exists() {
            return Ok(CodexImportResult {
                data_dir,
                data_dir_exists: false,
                scanned_files: 0,
                sessions: Vec::new(),
                requests: Vec::new(),
                skipped_incomplete_turns: 0,
            });
        }

        let mut files = Vec::new();
        collect_rollout_files(&data_dir, &mut files)?;
        files.sort();

        let mut sessions = Vec::new();
        let mut requests = Vec::new();
        let mut scanned: i64 = 0;

        for file in &files {
            if let Some(ref cutoff) = after_time {
                if let Ok(metadata) = fs::metadata(file) {
                    if let Ok(modified) = metadata.modified() {
                        let modified_dt: chrono::DateTime<chrono::Utc> = modified.into();
                        if modified_dt < *cutoff {
                            continue;
                        }
                    }
                }
            }
            scanned += 1;
            let parsed = parse_rollout_file(file)?;
            if let Some(session) = parsed.session {
                sessions.push(session);
            }
            requests.extend(parsed.requests);
        }

        Ok(CodexImportResult {
            data_dir,
            data_dir_exists: true,
            scanned_files: scanned,
            sessions,
            requests,
            skipped_incomplete_turns: 0,
        })
    }
}

pub fn default_codex_sessions_dir() -> Result<PathBuf, String> {
    let home_dir = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map_err(|error| error.to_string())?;
    Ok(PathBuf::from(home_dir).join(".codex").join("sessions"))
}

fn parse_rollout_file(path: &Path) -> Result<ParsedRollout, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let reader = BufReader::new(file);

    let mut session_meta: Option<SessionMeta> = None;
    let mut turns: HashMap<String, TurnState> = HashMap::new();
    let mut current_turn_id: Option<String> = None;

    for line in reader.lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }

        let envelope = match serde_json::from_str::<Value>(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let record_type = envelope.get("type").and_then(Value::as_str);
        let record_timestamp = envelope
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let payload = envelope.get("payload").unwrap_or(&Value::Null);

        match record_type {
            Some("session_meta") => {
                if let Some(meta) = parse_session_meta(payload) {
                    session_meta = Some(meta);
                }
            }
            Some("turn_context") => {
                if let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) {
                    let state = turns
                        .entry(turn_id.to_string())
                        .or_insert_with(|| TurnState {
                            turn_id: turn_id.to_string(),
                            ..TurnState::default()
                        });
                    state.cwd = payload
                        .get("cwd")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .or_else(|| state.cwd.clone());
                    state.model = payload
                        .get("model")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .or_else(|| state.model.clone());
                    state.timezone = payload
                        .get("timezone")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .or_else(|| state.timezone.clone());
                }
            }
            Some("event_msg") => {
                let event_type = payload.get("type").and_then(Value::as_str);
                match event_type {
                    Some("task_started") => {
                        if let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) {
                            let state =
                                turns
                                    .entry(turn_id.to_string())
                                    .or_insert_with(|| TurnState {
                                        turn_id: turn_id.to_string(),
                                        ..TurnState::default()
                                    });
                            current_turn_id = Some(turn_id.to_string());
                            if let Some(timestamp) = record_timestamp.clone() {
                                state.started_at = Some(timestamp);
                            }
                            state.status = Some("completed".to_string());
                        }
                    }
                    Some("token_count") => {
                        if let Some(turn_id) =
                            current_turn_id.clone().or_else(|| latest_turn_id(&turns))
                        {
                            let state = turns.get_mut(&turn_id).expect("latest turn must exist");
                            if let Some(info) = payload.get("info") {
                                if let Some(token_usage) = parse_token_usage(info) {
                                    state.token_usage = Some(token_usage);
                                    state.token_updates += 1;
                                }
                            }
                        }
                    }
                    Some("task_complete") => {
                        if let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) {
                            let state =
                                turns
                                    .entry(turn_id.to_string())
                                    .or_insert_with(|| TurnState {
                                        turn_id: turn_id.to_string(),
                                        ..TurnState::default()
                                    });
                            current_turn_id = Some(turn_id.to_string());
                            if let Some(timestamp) = record_timestamp.clone() {
                                state.finished_at = Some(timestamp);
                            }
                            state.duration_ms = payload.get("duration_ms").and_then(Value::as_i64);
                            state.ttft_ms = payload
                                .get("time_to_first_token_ms")
                                .and_then(Value::as_i64);
                            state.last_agent_message = payload
                                .get("last_agent_message")
                                .and_then(Value::as_str)
                                .map(str::to_owned);
                            state.status = Some("completed".to_string());
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(build_parsed_rollout(path, session_meta, turns))
}

fn build_parsed_rollout(
    path: &Path,
    session_meta: Option<SessionMeta>,
    turns: HashMap<String, TurnState>,
) -> ParsedRollout {
    let Some(session_meta) = session_meta else {
        return ParsedRollout {
            session: None,
            requests: Vec::new(),
        };
    };

    let mut requests = Vec::new();
    let source_file = path.display().to_string();
    let entrypoint = build_entrypoint(&session_meta);
    let mut finished_timestamps = Vec::new();
    let mut session_model: Option<String> = None;
    let mut session_cwd = session_meta.cwd.clone();

    let mut turn_list: Vec<TurnState> = turns.into_values().collect();
    turn_list.sort_by(|left, right| left.turn_id.cmp(&right.turn_id));

    for state in turn_list {
        let started_at = state
            .started_at
            .clone()
            .unwrap_or_else(|| session_meta.timestamp.clone());
        let finished_at = state.finished_at.clone();
        let token_usage = state.token_usage.clone().unwrap_or_default();
        let request_type = classify_codex_request_type(&state);
        let model = state.model.clone();
        let status = if finished_at.is_some() {
            state
                .status
                .clone()
                .unwrap_or_else(|| "completed".to_string())
        } else {
            "incomplete".to_string()
        };

        if session_model.is_none() {
            session_model = model.clone();
        }
        if session_cwd.is_none() {
            session_cwd = state.cwd.clone();
        }
        if let Some(timestamp) = finished_at.clone() {
            finished_timestamps.push(timestamp);
        }

        let request_summary_json = json!({
            "turnId": state.turn_id,
            "sourceFile": source_file,
            "cwd": state.cwd,
            "timezone": state.timezone,
            "tokenUpdates": state.token_updates,
            "requestType": request_type.as_str(),
        })
        .to_string();

        let response_summary_json = json!({
            "lastAgentMessage": state.last_agent_message,
            "modelProvider": session_meta.model_provider,
            "originator": session_meta.originator,
            "cliVersion": session_meta.cli_version,
        })
        .to_string();

        requests.push(RequestRecordUpsertRecord {
            id: format!("codex:{}:{}", session_meta.id, state.turn_id),
            provider: CODEX_PROVIDER.to_string(),
            source_mode: PASSIVE_SOURCE_MODE.to_string(),
            session_id: Some(session_meta.id.clone()),
            request_id: Some(state.turn_id),
            model,
            is_stream: request_type.is_stream(),
            input_tokens: token_usage.input_tokens,
            output_tokens: token_usage.output_tokens,
            cached_input_tokens: token_usage.cached_input_tokens,
            reasoning_tokens: token_usage.reasoning_tokens,
            ttft_ms: state.ttft_ms,
            duration_ms: state.duration_ms,
            status,
            started_at,
            finished_at,
            request_summary_json: Some(request_summary_json),
            response_summary_json: Some(response_summary_json),
            error_text: None,
        });
    }

    finished_timestamps.sort();
    let session_metadata_json = json!({
        "originator": session_meta.originator,
        "cliVersion": session_meta.cli_version,
        "source": session_meta.source,
        "threadSource": session_meta.thread_source,
        "modelProvider": session_meta.model_provider,
        "sourceFile": source_file,
    })
    .to_string();

    ParsedRollout {
        session: Some(SessionUpsertRecord {
            id: format!("codex:{}", session_meta.id),
            provider: CODEX_PROVIDER.to_string(),
            source_mode: PASSIVE_SOURCE_MODE.to_string(),
            session_id: session_meta.id,
            cwd: session_cwd,
            model: session_model,
            entrypoint,
            started_at: Some(session_meta.timestamp),
            finished_at: finished_timestamps.last().cloned(),
            metadata_json: Some(session_metadata_json),
        }),
        requests,
    }
}

fn classify_codex_request_type(state: &TurnState) -> RequestType {
    if state.ttft_ms.is_some() || state.token_updates > 1 {
        RequestType::Stream
    } else {
        RequestType::Unknown
    }
}

fn parse_session_meta(payload: &Value) -> Option<SessionMeta> {
    Some(SessionMeta {
        id: payload.get("id")?.as_str()?.to_string(),
        timestamp: payload.get("timestamp")?.as_str()?.to_string(),
        cwd: payload
            .get("cwd")
            .and_then(Value::as_str)
            .map(str::to_owned),
        originator: payload
            .get("originator")
            .and_then(Value::as_str)
            .map(str::to_owned),
        cli_version: payload
            .get("cli_version")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source: payload
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_owned),
        thread_source: payload
            .get("thread_source")
            .and_then(Value::as_str)
            .map(str::to_owned),
        model_provider: payload
            .get("model_provider")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

fn parse_token_usage(info: &Value) -> Option<TokenUsage> {
    let token_usage = info
        .get("last_token_usage")
        .or_else(|| info.get("total_token_usage"))?;

    Some(TokenUsage {
        input_tokens: token_usage
            .get("input_tokens")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        cached_input_tokens: token_usage
            .get("cached_input_tokens")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        output_tokens: token_usage
            .get("output_tokens")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        reasoning_tokens: token_usage
            .get("reasoning_output_tokens")
            .and_then(Value::as_i64)
            .unwrap_or(0),
    })
}

fn build_entrypoint(session_meta: &SessionMeta) -> Option<String> {
    match (
        session_meta.source.as_deref(),
        session_meta.originator.as_deref(),
    ) {
        (Some(source), Some(originator)) => Some(format!("{source}:{originator}")),
        (Some(source), None) => Some(source.to_string()),
        (None, Some(originator)) => Some(originator.to_string()),
        (None, None) => None,
    }
}

fn latest_turn_id(turns: &HashMap<String, TurnState>) -> Option<String> {
    turns.keys().max().map(|turn_id| turn_id.to_string())
}

fn collect_rollout_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            collect_rollout_files(&path, files)?;
            continue;
        }
        if file_type.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
        {
            files.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_rollout_file;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_completed_turn_and_stores_incomplete_turn() {
        let file_path = unique_test_path("codex-rollout");
        let content = [
            r#"{"timestamp":"2026-05-18T07:58:47.727Z","type":"session_meta","payload":{"id":"session-1","timestamp":"2026-05-18T07:58:47.727Z","cwd":"d:\\Projects\\Countdown","originator":"codex_vscode","cli_version":"0.131.0","source":"vscode","thread_source":"user","model_provider":"openai"}}"#,
            r#"{"timestamp":"2026-05-18T07:58:51.053Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-1"}}"#,
            r#"{"timestamp":"2026-05-18T07:58:51.054Z","type":"turn_context","payload":{"turn_id":"turn-1","cwd":"d:\\Projects\\Countdown","timezone":"Asia/Shanghai","model":"gpt-5.4"}}"#,
            r#"{"timestamp":"2026-05-18T07:58:52.000Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":1200,"cached_input_tokens":300,"output_tokens":240,"reasoning_output_tokens":50}}}}"#,
            r#"{"timestamp":"2026-05-18T07:58:54.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-1","duration_ms":2947,"time_to_first_token_ms":641,"last_agent_message":"done"}}"#,
            r#"{"timestamp":"2026-05-18T07:59:01.053Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn-2"}}"#,
            r#"{"timestamp":"2026-05-18T07:59:01.054Z","type":"turn_context","payload":{"turn_id":"turn-2","cwd":"d:\\Projects\\Countdown","timezone":"Asia/Shanghai","model":"gpt-5.4-mini"}}"#,
        ]
        .join("\n");

        fs::write(&file_path, content).expect("test rollout must be written");

        let parsed = parse_rollout_file(&file_path).expect("rollout parsing must succeed");

        fs::remove_file(&file_path).ok();

        let session = parsed.session.expect("session metadata must exist");
        assert_eq!(session.session_id, "session-1");
        assert_eq!(parsed.requests.len(), 2);

        let completed = &parsed.requests[0];
        assert_eq!(completed.request_id.as_deref(), Some("turn-1"));
        assert_eq!(completed.model.as_deref(), Some("gpt-5.4"));
        assert!(completed.is_stream);
        assert_eq!(completed.input_tokens, 1200);
        assert_eq!(completed.cached_input_tokens, 300);
        assert_eq!(completed.output_tokens, 240);
        assert_eq!(completed.reasoning_tokens, 50);
        assert_eq!(completed.ttft_ms, Some(641));
        assert_eq!(completed.duration_ms, Some(2947));
        assert_eq!(completed.status, "completed");

        let incomplete = &parsed.requests[1];
        assert_eq!(incomplete.request_id.as_deref(), Some("turn-2"));
        assert_eq!(incomplete.model.as_deref(), Some("gpt-5.4-mini"));
        assert!(!incomplete.is_stream);
        assert_eq!(incomplete.status, "incomplete");
        assert!(incomplete.finished_at.is_none());
    }

    fn unique_test_path(prefix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be available")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{timestamp}.jsonl"))
    }
}
