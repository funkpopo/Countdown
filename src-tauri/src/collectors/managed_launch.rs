use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::collectors::claude_code::CLAUDE_PROVIDER;
use crate::collectors::codex::CODEX_PROVIDER;
use crate::models::{
    ManagedLaunchInput, ManagedLaunchResult, RequestRecordUpsertRecord, RequestType,
    SessionUpsertRecord,
};

pub const MANAGED_SOURCE_MODE: &str = "managed_launch";

#[derive(Debug, Clone)]
pub struct ManagedLaunchCapture {
    pub result: ManagedLaunchResult,
    pub session: SessionUpsertRecord,
    pub request: RequestRecordUpsertRecord,
}

#[derive(Debug, Clone, Default)]
struct UsageCapture {
    session_id: Option<String>,
    request_id: Option<String>,
    model: Option<String>,
    input_tokens: i64,
    output_tokens: i64,
    cached_input_tokens: i64,
    reasoning_tokens: i64,
    ttft_ms: Option<i64>,
    is_stream: bool,
    response_summary: Option<String>,
}

pub fn run_managed_launch(input: ManagedLaunchInput) -> Result<ManagedLaunchCapture, String> {
    let provider = normalize_provider(&input.provider)?;
    if input.executable.trim().is_empty() {
        return Err("Executable is required.".to_string());
    }

    let session_id = format!("managed-{}", Uuid::new_v4());
    let request_id = Uuid::new_v4().to_string();
    let started_at = Utc::now();
    let start = Instant::now();

    let mut command = Command::new(&input.executable);
    command.args(&input.args);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    if input.stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    if let Some(cwd) = input.cwd.as_deref().filter(|cwd| !cwd.trim().is_empty()) {
        command.current_dir(cwd);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to start {}: {error}", input.executable))?;

    if let Some(stdin) = input.stdin.as_deref() {
        if let Some(mut child_stdin) = child.stdin.take() {
            child_stdin
                .write_all(stdin.as_bytes())
                .map_err(|error| format!("Failed to write process stdin: {error}"))?;
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("Failed to wait for managed launch: {error}"))?;
    let duration_ms = start.elapsed().as_millis() as i64;
    let finished_at = Utc::now();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let parsed = parse_managed_output(&provider, &stdout);
    let request_type = classify_managed_request_type(&parsed);
    let status = if output.status.success() {
        "completed"
    } else {
        "error"
    }
    .to_string();

    let model = parsed.model.clone().or(input.model.clone());
    let session_id = parsed.session_id.unwrap_or(session_id);
    let request_id = parsed.request_id.unwrap_or(request_id);

    let request_summary_json = json!({
        "executable": input.executable,
        "args": input.args,
        "cwd": input.cwd,
        "startedAt": started_at.to_rfc3339(),
        "exitCode": output.status.code(),
    })
    .to_string();

    let response_summary_json = json!({
        "stdoutBytes": output.stdout.len(),
        "stderrBytes": output.stderr.len(),
        "parsed": parsed.response_summary,
    })
    .to_string();

    let record_id = format!("{provider}:managed:{session_id}:{request_id}");
    let request = RequestRecordUpsertRecord {
        id: record_id,
        provider: provider.clone(),
        source_mode: MANAGED_SOURCE_MODE.to_string(),
        session_id: Some(session_id.clone()),
        request_id: Some(request_id.clone()),
        model: model.clone(),
        is_stream: request_type.is_stream(),
        input_tokens: parsed.input_tokens,
        output_tokens: parsed.output_tokens,
        cached_input_tokens: parsed.cached_input_tokens,
        reasoning_tokens: parsed.reasoning_tokens,
        ttft_ms: parsed.ttft_ms,
        duration_ms: Some(duration_ms),
        status: status.clone(),
        started_at: started_at.to_rfc3339(),
        finished_at: Some(finished_at.to_rfc3339()),
        request_summary_json: Some(request_summary_json),
        response_summary_json: Some(response_summary_json),
        error_text: if output.status.success() {
            None
        } else {
            Some(stderr.clone())
        },
    };

    let session = SessionUpsertRecord {
        id: format!("{provider}:managed:{session_id}"),
        provider: provider.clone(),
        source_mode: MANAGED_SOURCE_MODE.to_string(),
        session_id: session_id.clone(),
        cwd: input.cwd,
        model: model.clone(),
        entrypoint: Some(input.executable),
        started_at: Some(started_at.to_rfc3339()),
        finished_at: Some(finished_at.to_rfc3339()),
        metadata_json: Some(json!({ "args": input.args }).to_string()),
    };

    let result = ManagedLaunchResult {
        provider,
        source_mode: MANAGED_SOURCE_MODE.to_string(),
        session_id,
        request_id,
        status,
        exit_code: output.status.code(),
        model,
        input_tokens: request.input_tokens,
        output_tokens: request.output_tokens,
        cached_input_tokens: request.cached_input_tokens,
        reasoning_tokens: request.reasoning_tokens,
        ttft_ms: request.ttft_ms,
        duration_ms,
        stdout,
        stderr,
    };

    Ok(ManagedLaunchCapture {
        result,
        session,
        request,
    })
}

fn classify_managed_request_type(parsed: &UsageCapture) -> RequestType {
    if parsed.is_stream || parsed.ttft_ms.is_some() {
        RequestType::Stream
    } else {
        RequestType::Unknown
    }
}

fn normalize_provider(provider: &str) -> Result<String, String> {
    match provider.trim() {
        CODEX_PROVIDER | "codex_sdk" => Ok(CODEX_PROVIDER.to_string()),
        CLAUDE_PROVIDER | "claude" => Ok(CLAUDE_PROVIDER.to_string()),
        other => Err(format!("Unsupported managed launch provider: {other}")),
    }
}

fn parse_managed_output(provider: &str, stdout: &str) -> UsageCapture {
    let mut capture = UsageCapture::default();

    for value in parse_json_values(stdout) {
        merge_usage_value(provider, &mut capture, &value);
    }

    capture
}

fn parse_json_values(stdout: &str) -> Vec<Value> {
    let trimmed = stdout.trim();
    let mut values = Vec::new();

    if !trimmed.is_empty() {
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            values.push(value);
            return values;
        }
    }

    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            values.push(value);
        }
    }

    values
}

fn merge_usage_value(provider: &str, capture: &mut UsageCapture, value: &Value) {
    capture.response_summary = Some(value.to_string());

    if capture.session_id.is_none() {
        capture.session_id = first_string(value, ["session_id", "sessionId", "session"].as_slice());
    }
    if capture.request_id.is_none() {
        capture.request_id = first_string(value, ["request_id", "requestId", "id", "turn_id"].as_slice());
    }
    if capture.model.is_none() {
        capture.model = first_string(value, ["model"].as_slice());
    }

    if let Some(ms) = first_i64(
        value,
        [
            "ttft_ms",
            "ttftMs",
            "time_to_first_token_ms",
            "timeToFirstTokenMs",
        ].as_slice(),
    ) {
        capture.ttft_ms = Some(ms);
    }

    let candidate = value
        .get("usage")
        .or_else(|| value.get("token_usage"))
        .or_else(|| value.get("last_token_usage"))
        .or_else(|| value.get("total_token_usage"))
        .or_else(|| value.pointer("/payload/info/last_token_usage"))
        .or_else(|| value.pointer("/payload/info/total_token_usage"))
        .unwrap_or(value);

    let input = first_i64(
        candidate,
        [
            "input_tokens",
            "inputTokens",
            "prompt_tokens",
            "promptTokens",
        ].as_slice(),
    );
    let output = first_i64(
        candidate,
        [
            "output_tokens",
            "outputTokens",
            "completion_tokens",
            "completionTokens",
        ].as_slice(),
    );
    let cached = first_i64(
        candidate,
        [
            "cached_input_tokens",
            "cachedInputTokens",
            "cache_read_input_tokens",
            "cacheReadInputTokens",
        ].as_slice(),
    )
    .unwrap_or(0)
        + first_i64(
            candidate,
            ["cache_creation_input_tokens", "cacheCreationInputTokens"].as_slice(),
        )
        .unwrap_or(0);
    let reasoning = first_i64(
        candidate,
        [
            "reasoning_tokens",
            "reasoningTokens",
            "reasoning_output_tokens",
            "reasoningOutputTokens",
        ].as_slice(),
    );

    if let Some(input) = input {
        capture.input_tokens = input;
    }
    if let Some(output) = output {
        capture.output_tokens = output;
    }
    if cached > 0 {
        capture.cached_input_tokens = cached;
    }
    if let Some(reasoning) = reasoning {
        capture.reasoning_tokens = reasoning;
    }

    let event_type = value
        .get("type")
        .or_else(|| value.pointer("/payload/type"))
        .and_then(Value::as_str);
    if matches!(
        event_type,
        Some("token_count")
            | Some("content_block_delta")
            | Some("message_delta")
            | Some("response.output_text.delta")
            | Some("turn.completed")
            | Some("thread/tokenUsage/updated")
    ) {
        capture.is_stream = true;
    }

    if provider == CLAUDE_PROVIDER {
        if let Some(message) = value.get("message") {
            if capture.model.is_none() {
                capture.model = first_string(message, ["model"].as_slice());
            }
            if let Some(usage) = message.get("usage") {
                merge_usage_value(provider, capture, usage);
            }
        }
    }
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(Value::as_str) {
            return Some(text.to_string());
        }
        if let Some(text) = value
            .pointer(&format!("/payload/{key}"))
            .and_then(Value::as_str)
        {
            return Some(text.to_string());
        }
    }
    None
}

fn first_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        if let Some(number) = value.get(*key).and_then(Value::as_i64) {
            return Some(number);
        }
        if let Some(number) = value
            .pointer(&format!("/payload/{key}"))
            .and_then(Value::as_i64)
        {
            return Some(number);
        }
    }
    None
}
