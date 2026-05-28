use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::DateTime;
use serde_json::{json, Value};

use crate::models::{RequestRecordUpsertRecord, RequestType, SessionUpsertRecord};

pub const CLAUDE_PROVIDER: &str = "claude_code";
pub const PASSIVE_SOURCE_MODE: &str = "passive_ingest";

#[derive(Debug, Clone, Default)]
pub struct ClaudeCodeCollector;

#[derive(Debug, Clone)]
pub struct ClaudeImportResult {
    pub data_dir: PathBuf,
    pub data_dir_exists: bool,
    pub scanned_files: i64,
    pub sessions: Vec<SessionUpsertRecord>,
    pub requests: Vec<RequestRecordUpsertRecord>,
    pub skipped_incomplete_sessions: i64,
}

#[derive(Debug, Clone, Default)]
struct MessageUsage {
    input_tokens: i64,
    cache_read_input_tokens: i64,
    cache_creation_input_tokens: i64,
    output_tokens: i64,
}

#[derive(Debug, Clone)]
struct AssistantMessage {
    started_at: Option<String>,
    timestamp: String,
    model: Option<String>,
    request_type: RequestType,
    usage: Option<MessageUsage>,
    content_summary: Option<String>,
    has_tool_use: bool,
}

#[derive(Debug, Clone, Default)]
struct SessionState {
    session_id: String,
    cwd: Option<String>,
    entrypoint: Option<String>,
    first_timestamp: Option<String>,
    last_timestamp: Option<String>,
    assistant_messages: Vec<AssistantMessage>,
    tool_durations: Vec<i64>,
}

#[derive(Debug, Clone)]
struct ParsedProjectFile {
    sessions: HashMap<String, SessionState>,
}

impl ClaudeCodeCollector {
    pub fn import_default_sessions() -> Result<ClaudeImportResult, String> {
        Self::import_sessions_since(None)
    }

    pub fn import_sessions_since(after_time: Option<chrono::DateTime<chrono::Utc>>) -> Result<ClaudeImportResult, String> {
        let data_dir = default_claude_data_dir()?;
        if !data_dir.exists() {
            return Ok(ClaudeImportResult {
                data_dir,
                data_dir_exists: false,
                scanned_files: 0,
                sessions: Vec::new(),
                requests: Vec::new(),
                skipped_incomplete_sessions: 0,
            });
        }

        let projects_dir = data_dir.join("projects");
        let sessions_dir = data_dir.join("sessions");

        let mut files = Vec::new();
        if projects_dir.exists() {
            collect_jsonl_files(&projects_dir, &mut files)?;
        }
        files.sort();

        let session_meta_overrides = load_session_meta_overrides(&sessions_dir);

        let mut all_sessions: HashMap<String, SessionState> = HashMap::new();
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
            let parsed = parse_project_jsonl(file)?;
            for (key, state) in parsed.sessions {
                all_sessions
                    .entry(key)
                    .and_modify(|existing| merge_session_state(existing, state.clone()))
                    .or_insert(state);
            }
        }

        let mut sessions = Vec::new();
        let mut requests = Vec::new();

        for (key, mut state) in all_sessions {
            if let Some(override_meta) = session_meta_overrides.get(&state.session_id) {
                if state.cwd.is_none() {
                    state.cwd = override_meta.cwd.clone();
                }
                if state.entrypoint.is_none() {
                    state.entrypoint = override_meta.entrypoint.clone();
                }
                if state.first_timestamp.is_none() {
                    state.first_timestamp = override_meta.started_at.clone();
                }
            }

            state
                .assistant_messages
                .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

            let source_file = files
                .iter()
                .find(|f| f.file_stem().and_then(|s| s.to_str()) == Some(&key))
                .map(|f| f.display().to_string())
                .unwrap_or_default();

            let session_record = build_session_record(&key, &state, &source_file);
            sessions.push(session_record);

            for msg in &state.assistant_messages {
                requests.push(build_request_record(&key, &state, msg, &source_file));
            }
        }

        Ok(ClaudeImportResult {
            data_dir,
            data_dir_exists: true,
            scanned_files: scanned,
            sessions,
            requests,
            skipped_incomplete_sessions: 0,
        })
    }
}

pub fn default_claude_data_dir() -> Result<PathBuf, String> {
    let home_dir = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .map_err(|error| error.to_string())?;
    Ok(PathBuf::from(home_dir).join(".claude"))
}

fn collect_jsonl_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            collect_jsonl_files(&path, files)?;
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

fn parse_project_jsonl(path: &Path) -> Result<ParsedProjectFile, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let reader = BufReader::new(file);

    let mut sessions: HashMap<String, SessionState> = HashMap::new();
    let mut last_response_trigger_timestamps: HashMap<String, String> = HashMap::new();
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

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
        let timestamp = envelope
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_owned);

        match record_type {
            Some("user") => {
                let session_id = envelope
                    .get("sessionId")
                    .or_else(|| envelope.get("session_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| file_stem.clone());

                let state = sessions
                    .entry(session_id.clone())
                    .or_insert_with(|| SessionState {
                        session_id: session_id.clone(),
                        ..SessionState::default()
                    });

                if state.first_timestamp.is_none() {
                    if let Some(ts) = &timestamp {
                        state.first_timestamp = Some(ts.clone());
                    }
                }

                if let Some(cwd) = envelope.get("cwd").and_then(Value::as_str) {
                    state.cwd = Some(cwd.to_string());
                }

                if let Some(originator) = envelope.get("originator").and_then(Value::as_str) {
                    state.entrypoint = Some(originator.to_string());
                }

                if let Some(ts) = timestamp {
                    last_response_trigger_timestamps.insert(session_id, ts);
                }
            }
            Some("assistant") => {
                let session_id = envelope
                    .get("sessionId")
                    .or_else(|| envelope.get("session_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| file_stem.clone());

                let state = sessions
                    .entry(session_id.clone())
                    .or_insert_with(|| SessionState {
                        session_id: session_id.clone(),
                        ..SessionState::default()
                    });

                if let Some(ts) = &timestamp {
                    state.last_timestamp = Some(ts.clone());
                }

                let message = envelope.get("message");
                let model = message
                    .and_then(|m| m.get("model"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);

                let usage = message
                    .and_then(|m| m.get("usage"))
                    .and_then(parse_assistant_usage);

                let content_summary = message
                    .and_then(|m| m.get("content"))
                    .and_then(summarize_content);

                let has_tool_use = message
                    .and_then(|m| m.get("content"))
                    .map(|c| content_has_tool_use(c))
                    .unwrap_or(false);
                let request_type = classify_claude_code_record_type(&envelope);

                if let Some(ts) = timestamp.clone() {
                    let started_at = last_response_trigger_timestamps
                        .get(&session_id)
                        .cloned()
                        .or_else(|| state.first_timestamp.clone());

                    state.assistant_messages.push(AssistantMessage {
                        started_at,
                        timestamp: ts,
                        model,
                        request_type,
                        usage,
                        content_summary,
                        has_tool_use,
                    });
                }
            }
            Some("tool_result") | Some("tool_use_result") => {
                let session_id = envelope
                    .get("sessionId")
                    .or_else(|| envelope.get("session_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| file_stem.clone());

                let state = sessions
                    .entry(session_id.clone())
                    .or_insert_with(|| SessionState {
                        session_id: session_id.clone(),
                        ..SessionState::default()
                    });

                if let Some(duration_ms) = envelope
                    .get("durationMs")
                    .or_else(|| envelope.get("duration_ms"))
                    .and_then(Value::as_i64)
                {
                    state.tool_durations.push(duration_ms);
                }

                if let Some(ts) = timestamp {
                    last_response_trigger_timestamps.insert(session_id, ts);
                }
            }
            _ => {}
        }
    }

    Ok(ParsedProjectFile { sessions })
}

fn parse_assistant_usage(usage: &Value) -> Option<MessageUsage> {
    let input_tokens = usage
        .get("input_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let cache_creation = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    if input_tokens == 0 && cache_read == 0 && cache_creation == 0 && output_tokens == 0 {
        return None;
    }

    Some(MessageUsage {
        input_tokens,
        cache_read_input_tokens: cache_read,
        cache_creation_input_tokens: cache_creation,
        output_tokens,
    })
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

fn summarize_content(content: &Value) -> Option<String> {
    match content {
        Value::String(s) => Some(truncate_utf8(s, 200)),
        Value::Array(arr) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string())
                        .or_else(|| {
                            item.get("type")
                                .and_then(Value::as_str)
                                .map(|t| format!("[{}]", t))
                        })
                })
                .collect();
            let joined = parts.join(" ");
            if joined.is_empty() {
                None
            } else {
                Some(truncate_utf8(&joined, 200))
            }
        }
        _ => None,
    }
}

fn content_has_tool_use(content: &Value) -> bool {
    match content {
        Value::Array(arr) => arr.iter().any(|item| {
            item.get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "tool_use")
        }),
        _ => false,
    }
}

fn classify_claude_code_record_type(envelope: &Value) -> RequestType {
    if explicit_stream_flag(envelope).unwrap_or(false) {
        return RequestType::Stream;
    }

    let record_type = envelope.get("type").and_then(Value::as_str);
    let message_type = envelope
        .get("message")
        .and_then(|message| message.get("type"))
        .and_then(Value::as_str);

    if matches!(
        record_type.or(message_type),
        Some("content_block_delta")
            | Some("message_delta")
            | Some("message_start")
            | Some("message_stop")
            | Some("input_json_delta")
            | Some("ping")
    ) {
        return RequestType::Stream;
    }

    if record_type == Some("assistant") {
        return RequestType::Sync;
    }

    RequestType::Unknown
}

fn explicit_stream_flag(value: &Value) -> Option<bool> {
    const PATHS: [&str; 8] = [
        "/is_stream",
        "/isStream",
        "/stream",
        "/request/stream",
        "/request/is_stream",
        "/request/isStream",
        "/message/stream",
        "/message/is_stream",
    ];

    for path in PATHS {
        if let Some(flag) = value.pointer(path).and_then(Value::as_bool) {
            return Some(flag);
        }
    }

    None
}

fn infer_elapsed_ms(started_at: Option<&str>, finished_at: &str) -> Option<i64> {
    let started_at = started_at?;
    let start = DateTime::parse_from_rfc3339(started_at).ok()?;
    let finish = DateTime::parse_from_rfc3339(finished_at).ok()?;
    let elapsed = finish.signed_duration_since(start).num_milliseconds();

    if elapsed >= 0 {
        Some(elapsed)
    } else {
        None
    }
}

fn build_request_record(
    session_key: &str,
    state: &SessionState,
    msg: &AssistantMessage,
    source_file: &str,
) -> RequestRecordUpsertRecord {
    let usage = msg.usage.clone().unwrap_or_default();
    let inferred_duration_ms = infer_elapsed_ms(msg.started_at.as_deref(), &msg.timestamp);
    let started_at = msg
        .started_at
        .clone()
        .unwrap_or_else(|| msg.timestamp.clone());
    let request_type = msg.request_type;

    let request_summary_json = json!({
        "sessionId": state.session_id,
        "sourceFile": source_file,
        "cwd": state.cwd,
        "hasToolUse": msg.has_tool_use,
        "requestType": request_type.as_str(),
        "typeSource": "claude_code_jsonl_record",
        "ttftSource": "unavailable_passive_jsonl",
        "durationSource": if inferred_duration_ms.is_some() { "inferred_from_jsonl_timestamps" } else { "unavailable" },
    })
    .to_string();

    let response_summary_json = json!({
        "model": msg.model,
        "contentSummary": msg.content_summary,
        "entrypoint": state.entrypoint,
    })
    .to_string();

    let request_id = format!("claude:{session_key}:{}", msg.timestamp);

    RequestRecordUpsertRecord {
        id: request_id.clone(),
        provider: CLAUDE_PROVIDER.to_string(),
        source_mode: PASSIVE_SOURCE_MODE.to_string(),
        session_id: Some(session_key.to_string()),
        request_id: Some(msg.timestamp.clone()),
        model: msg.model.clone(),
        is_stream: request_type.is_stream(),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cached_input_tokens: usage.cache_read_input_tokens + usage.cache_creation_input_tokens,
        reasoning_tokens: 0,
        ttft_ms: None,
        duration_ms: inferred_duration_ms,
        status: "completed".to_string(),
        started_at,
        finished_at: Some(msg.timestamp.clone()),
        request_summary_json: Some(request_summary_json),
        response_summary_json: Some(response_summary_json),
        error_text: None,
    }
}

fn build_session_record(key: &str, state: &SessionState, source_file: &str) -> SessionUpsertRecord {
    let metadata_json = json!({
        "sourceFile": source_file,
        "cwd": state.cwd,
        "entrypoint": state.entrypoint,
        "toolCallCount": state.tool_durations.len(),
        "totalToolDurationMs": state.tool_durations.iter().sum::<i64>(),
        "messageCount": state.assistant_messages.len(),
    })
    .to_string();

    SessionUpsertRecord {
        id: format!("claude:{}", key),
        provider: CLAUDE_PROVIDER.to_string(),
        source_mode: PASSIVE_SOURCE_MODE.to_string(),
        session_id: key.to_string(),
        cwd: state.cwd.clone(),
        model: state
            .assistant_messages
            .iter()
            .find_map(|m| m.model.clone()),
        entrypoint: state.entrypoint.clone(),
        started_at: state.first_timestamp.clone(),
        finished_at: state.last_timestamp.clone(),
        metadata_json: Some(metadata_json),
    }
}

fn merge_session_state(existing: &mut SessionState, incoming: SessionState) {
    if existing.cwd.is_none() {
        existing.cwd = incoming.cwd;
    }
    if existing.entrypoint.is_none() {
        existing.entrypoint = incoming.entrypoint;
    }
    if existing.first_timestamp.is_none() {
        existing.first_timestamp = incoming.first_timestamp;
    }
    existing.last_timestamp = incoming
        .last_timestamp
        .or_else(|| existing.last_timestamp.clone());
    existing
        .assistant_messages
        .extend(incoming.assistant_messages);
    existing.tool_durations.extend(incoming.tool_durations);
}

#[derive(Debug, Clone)]
struct SessionMetaOverride {
    cwd: Option<String>,
    entrypoint: Option<String>,
    started_at: Option<String>,
}

fn load_session_meta_overrides(sessions_dir: &Path) -> HashMap<String, SessionMetaOverride> {
    let mut overrides = HashMap::new();
    if !sessions_dir.exists() {
        return overrides;
    }

    let entries = match fs::read_dir(sessions_dir) {
        Ok(entries) => entries,
        Err(_) => return overrides,
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let parsed: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let session_id = parsed
            .get("sessionId")
            .or_else(|| parsed.get("session_id"))
            .and_then(Value::as_str)
            .map(str::to_owned);

        if let Some(sid) = session_id {
            overrides.insert(
                sid,
                SessionMetaOverride {
                    cwd: parsed.get("cwd").and_then(Value::as_str).map(str::to_owned),
                    entrypoint: parsed
                        .get("entrypoint")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    started_at: parsed
                        .get("startedAt")
                        .or_else(|| parsed.get("started_at"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                },
            );
        }
    }

    overrides
}

#[cfg(test)]
mod tests {
    use super::{
        build_request_record, classify_claude_code_record_type, parse_project_jsonl,
        summarize_content, truncate_utf8, AssistantMessage, MessageUsage, SessionState,
    };
    use crate::models::RequestType;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_assistant_messages_and_skips_empty_session() {
        let file_path = unique_test_path("claude-project");
        let content = [
            r#"{"type":"user","timestamp":"2026-05-18T08:00:00.000Z","sessionId":"claude-session-1","cwd":"d:\\Projects\\Countdown","originator":"claude_vscode"}"#,
            r#"{"type":"assistant","timestamp":"2026-05-18T08:00:05.000Z","sessionId":"claude-session-1","message":{"model":"claude-sonnet-4-20250514","role":"assistant","content":"Here is the code.","usage":{"input_tokens":500,"cache_read_input_tokens":100,"cache_creation_input_tokens":50,"output_tokens":200}}}"#,
            r#"{"type":"assistant","timestamp":"2026-05-18T08:00:10.000Z","sessionId":"claude-session-1","message":{"model":"claude-sonnet-4-20250514","role":"assistant","content":[{"type":"text","text":"Updated."},{"type":"tool_use","id":"tool-1","name":"write"}],"usage":{"input_tokens":600,"cache_read_input_tokens":150,"cache_creation_input_tokens":0,"output_tokens":150}}}"#,
            r#"{"type":"tool_result","timestamp":"2026-05-18T08:00:12.000Z","sessionId":"claude-session-1","durationMs":45}"#,
        ]
        .join("\n");

        fs::write(&file_path, content).expect("test file must be written");

        let parsed = parse_project_jsonl(&file_path).expect("parsing must succeed");

        fs::remove_file(&file_path).ok();

        assert_eq!(parsed.sessions.len(), 1);
        let state = parsed
            .sessions
            .get("claude-session-1")
            .expect("session must exist");
        assert_eq!(state.assistant_messages.len(), 2);
        assert_eq!(state.cwd.as_deref(), Some("d:\\Projects\\Countdown"));
        assert_eq!(state.entrypoint.as_deref(), Some("claude_vscode"));

        let first_msg = &state.assistant_messages[0];
        assert_eq!(
            first_msg.started_at.as_deref(),
            Some("2026-05-18T08:00:00.000Z")
        );
        assert_eq!(first_msg.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(
            super::infer_elapsed_ms(first_msg.started_at.as_deref(), &first_msg.timestamp),
            Some(5000)
        );
        let usage = first_msg.usage.as_ref().expect("usage must exist");
        assert_eq!(usage.input_tokens, 500);
        assert_eq!(usage.cache_read_input_tokens, 100);
        assert_eq!(usage.cache_creation_input_tokens, 50);
        assert_eq!(usage.output_tokens, 200);
        assert!(!first_msg.has_tool_use);

        let second_msg = &state.assistant_messages[1];
        assert_eq!(
            second_msg.started_at.as_deref(),
            Some("2026-05-18T08:00:00.000Z")
        );
        assert!(second_msg.has_tool_use);
        assert_eq!(state.tool_durations.len(), 1);
        assert_eq!(state.tool_durations[0], 45);
    }

    #[test]
    fn parses_session_without_assistant_messages() {
        let file_path = unique_test_path("claude-empty");
        let content =
            r#"{"type":"user","timestamp":"2026-05-18T08:00:00.000Z","sessionId":"empty-session"}"#;

        fs::write(&file_path, content).expect("test file must be written");

        let parsed = parse_project_jsonl(&file_path).expect("parsing must succeed");

        fs::remove_file(&file_path).ok();

        assert_eq!(parsed.sessions.len(), 1);
        let state = parsed
            .sessions
            .get("empty-session")
            .expect("session must exist");
        assert!(state.assistant_messages.is_empty());
    }

    #[test]
    fn passive_records_keep_duration_and_leave_ttft_empty() {
        let state = SessionState {
            session_id: "claude-session-1".to_string(),
            cwd: Some("d:\\Projects\\Countdown".to_string()),
            entrypoint: Some("claude_vscode".to_string()),
            ..SessionState::default()
        };
        let message = AssistantMessage {
            started_at: Some("2026-05-18T08:00:00.000Z".to_string()),
            timestamp: "2026-05-18T08:00:05.000Z".to_string(),
            model: Some("claude-sonnet-4-20250514".to_string()),
            request_type: RequestType::Sync,
            usage: Some(MessageUsage {
                input_tokens: 500,
                cache_read_input_tokens: 100,
                cache_creation_input_tokens: 50,
                output_tokens: 200,
            }),
            content_summary: Some("Here is the code.".to_string()),
            has_tool_use: false,
        };

        let record = build_request_record("claude-session-1", &state, &message, "source.jsonl");

        assert_eq!(record.ttft_ms, None);
        assert_eq!(record.duration_ms, Some(5000));
        assert!(!record.is_stream);
        assert_eq!(record.started_at, "2026-05-18T08:00:00.000Z");
        assert_eq!(
            record.finished_at.as_deref(),
            Some("2026-05-18T08:00:05.000Z")
        );
    }

    #[test]
    fn passive_records_keep_type_independent_of_tool_or_cache_usage() {
        let state = SessionState {
            session_id: "claude-session-1".to_string(),
            ..SessionState::default()
        };
        let message = AssistantMessage {
            started_at: Some("2026-05-18T08:00:00.000Z".to_string()),
            timestamp: "2026-05-18T08:00:05.000Z".to_string(),
            model: Some("claude-sonnet-4-20250514".to_string()),
            request_type: RequestType::Sync,
            usage: Some(MessageUsage {
                input_tokens: 500,
                cache_read_input_tokens: 100,
                cache_creation_input_tokens: 50,
                output_tokens: 200,
            }),
            content_summary: Some("[tool_use]".to_string()),
            has_tool_use: true,
        };

        let record = build_request_record("claude-session-1", &state, &message, "source.jsonl");

        assert!(!record.is_stream);
        assert_eq!(record.cached_input_tokens, 150);
    }

    #[test]
    fn claude_code_assistant_messages_default_to_sync_when_no_stream_signal_exists() {
        let file_path = unique_test_path("claude-sync");
        let content = [
            r#"{"type":"user","timestamp":"2026-05-18T08:00:00.000Z","sessionId":"claude-session-1"}"#,
            r#"{"type":"assistant","timestamp":"2026-05-18T08:00:05.000Z","sessionId":"claude-session-1","message":{"model":"claude-sonnet-4-20250514","role":"assistant","content":"Done.","usage":{"input_tokens":1,"output_tokens":2}}}"#,
        ]
        .join("\n");

        fs::write(&file_path, content).expect("test file must be written");

        let parsed = parse_project_jsonl(&file_path).expect("parsing must succeed");

        fs::remove_file(&file_path).ok();

        let state = parsed
            .sessions
            .get("claude-session-1")
            .expect("session must exist");
        let message = state
            .assistant_messages
            .first()
            .expect("assistant message must exist");
        assert_eq!(message.request_type, RequestType::Sync);

        let record = build_request_record("claude-session-1", state, message, "source.jsonl");
        assert!(!record.is_stream);
        let summary: serde_json::Value = serde_json::from_str(
            record
                .request_summary_json
                .as_deref()
                .expect("request summary must exist"),
        )
        .expect("request summary must be valid json");
        assert_eq!(summary["requestType"], "sync");
        assert_eq!(summary["typeSource"], "claude_code_jsonl_record");
    }

    #[test]
    fn claude_code_explicit_stream_flags_are_respected() {
        let envelope = json!({
            "type": "assistant",
            "stream": true,
            "message": {
                "type": "message_delta"
            }
        });

        assert_eq!(
            classify_claude_code_record_type(&envelope),
            RequestType::Stream
        );
    }

    #[test]
    fn truncate_utf8_preserves_char_boundaries_for_multibyte_text() {
        let text = "我注意到你发了一条跟项目相关的消息，但不太确定上下文。你是让我查看当前项目（countdown）中时间记录相关代码并修复时区问题吗？能再描述一下具体需求吗？";

        let truncated = truncate_utf8(text, 200);

        assert!(truncated.ends_with("..."));
        assert!(truncated.is_char_boundary(truncated.len()));
        assert!(truncated.starts_with("我注意到你发了一条跟项目相关的消息"));
        assert!(truncated.len() < text.len() + 3);
    }

    #[test]
    fn summarize_content_truncates_multibyte_string_without_panicking() {
        let text = "我注意到你发了一条跟项目相关的消息，但不太确定上下文。你是让我查看当前项目（countdown）中时间记录相关代码并修复时区问题吗？能再描述一下具体需求吗？";

        let summary = summarize_content(&json!(text)).expect("summary must exist");

        assert!(summary.ends_with("..."));
        assert!(summary.is_char_boundary(summary.len()));
        assert!(summary.len() <= text.len() + 3);
    }

    fn unique_test_path(prefix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be available")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{timestamp}.jsonl"))
    }
}
