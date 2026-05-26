use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapInfo {
    pub product_name: String,
    pub version: String,
    pub identifier: String,
    pub app_data_dir: String,
    pub database_path: String,
    pub phase0_complete: bool,
    pub phase1_complete: bool,
    pub phase2_complete: bool,
    pub phase3_complete: bool,
    pub phase4_complete: bool,
    pub phase5_complete: bool,
    pub phase6_complete: bool,
    pub phase7_complete: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseHealth {
    pub database_path: String,
    pub exists: bool,
    pub writable: bool,
    pub schema_version: Option<String>,
    pub initialized_at: Option<String>,
    pub migration_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedMigration {
    pub version: i64,
    pub name: String,
    pub applied_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStat {
    pub table_name: String,
    pub row_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileRecord {
    pub id: String,
    pub provider_key: String,
    pub display_name: String,
    pub base_url: Option<String>,
    pub api_format: String,
    pub api_key_env: Option<String>,
    pub enabled: bool,
    pub extra_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfileUpsertInput {
    pub id: String,
    pub provider_key: String,
    pub display_name: String,
    pub base_url: Option<String>,
    pub api_format: String,
    pub api_key_env: Option<String>,
    pub enabled: bool,
    pub extra_json: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSummary {
    pub schema_version: Option<String>,
    pub initialized_at: Option<String>,
    pub applied_migrations: Vec<AppliedMigration>,
    pub tables: Vec<TableStat>,
    pub provider_profiles: Vec<ProviderProfileRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinedTodayUsage {
    pub date: String,
    pub claude_input_tokens: i64,
    pub claude_cached_input_tokens: i64,
    pub claude_output_tokens: i64,
    pub claude_total_tokens: i64,
    pub claude_request_count: i64,
    pub codex_input_tokens: i64,
    pub codex_cached_input_tokens: i64,
    pub codex_output_tokens: i64,
    pub codex_total_tokens: i64,
    pub codex_request_count: i64,
    pub combined_input_tokens: i64,
    pub combined_cached_input_tokens: i64,
    pub combined_output_tokens: i64,
    pub combined_total_tokens: i64,
    pub combined_request_count: i64,
    pub last_refresh_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateRangeInput {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinedUsage {
    pub start_date: String,
    pub end_date: String,
    pub claude_input_tokens: i64,
    pub claude_cached_input_tokens: i64,
    pub claude_output_tokens: i64,
    pub claude_total_tokens: i64,
    pub claude_request_count: i64,
    pub codex_input_tokens: i64,
    pub codex_cached_input_tokens: i64,
    pub codex_output_tokens: i64,
    pub codex_total_tokens: i64,
    pub codex_request_count: i64,
    pub combined_input_tokens: i64,
    pub combined_cached_input_tokens: i64,
    pub combined_output_tokens: i64,
    pub combined_total_tokens: i64,
    pub combined_request_count: i64,
    pub last_refresh_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistogramInput {
    pub period: String,
    pub granularity: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistogramBucket {
    pub bucket: String,
    pub label: String,
    pub claude_input_tokens: i64,
    pub claude_cached_input_tokens: i64,
    pub claude_output_tokens: i64,
    pub claude_total_tokens: i64,
    pub claude_request_count: i64,
    pub codex_input_tokens: i64,
    pub codex_cached_input_tokens: i64,
    pub codex_output_tokens: i64,
    pub codex_total_tokens: i64,
    pub codex_request_count: i64,
    pub combined_input_tokens: i64,
    pub combined_cached_input_tokens: i64,
    pub combined_output_tokens: i64,
    pub combined_total_tokens: i64,
    pub combined_request_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistogram {
    pub period: String,
    pub granularity: String,
    pub buckets: Vec<UsageHistogramBucket>,
    pub last_refresh_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsageRecord {
    pub date: String,
    pub provider: String,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub request_count: i64,
    pub stream_count: i64,
    pub non_stream_count: i64,
    pub avg_ttft_ms: Option<f64>,
    pub avg_duration_ms: Option<f64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRecordListItem {
    pub id: String,
    pub provider: String,
    pub source_mode: String,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub model: Option<String>,
    pub is_stream: bool,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub reasoning_tokens: i64,
    pub ttft_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub cwd: Option<String>,
    pub entrypoint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOverview {
    pub data_dir: String,
    pub data_dir_exists: bool,
    pub session_count: i64,
    pub request_count: i64,
    pub today_usage: Option<DailyUsageRecord>,
    pub recent_requests: Vec<RequestRecordListItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSyncSummary {
    pub data_dir: String,
    pub data_dir_exists: bool,
    pub scanned_files: i64,
    pub imported_sessions: i64,
    pub imported_requests: i64,
    pub skipped_incomplete_turns: i64,
    pub session_count: i64,
    pub request_count: i64,
    pub today_usage: Option<DailyUsageRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeOverview {
    pub data_dir: String,
    pub data_dir_exists: bool,
    pub session_count: i64,
    pub request_count: i64,
    pub today_usage: Option<DailyUsageRecord>,
    pub recent_requests: Vec<RequestRecordListItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeSyncSummary {
    pub data_dir: String,
    pub data_dir_exists: bool,
    pub scanned_files: i64,
    pub imported_sessions: i64,
    pub imported_requests: i64,
    pub skipped_incomplete_sessions: i64,
    pub session_count: i64,
    pub request_count: i64,
    pub today_usage: Option<DailyUsageRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedLaunchInput {
    pub provider: String,
    pub executable: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedLaunchResult {
    pub provider: String,
    pub source_mode: String,
    pub session_id: String,
    pub request_id: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub reasoning_tokens: i64,
    pub ttft_ms: Option<i64>,
    pub duration_ms: i64,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct SessionUpsertRecord {
    pub id: String,
    pub provider: String,
    pub source_mode: String,
    pub session_id: String,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub entrypoint: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RequestRecordUpsertRecord {
    pub id: String,
    pub provider: String,
    pub source_mode: String,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub model: Option<String>,
    pub is_stream: bool,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub reasoning_tokens: i64,
    pub ttft_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub request_summary_json: Option<String>,
    pub response_summary_json: Option<String>,
    pub error_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestType {
    Unknown,
    Sync,
    Stream,
}

impl RequestType {
    pub fn is_stream(self) -> bool {
        matches!(self, Self::Stream)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Sync => "sync",
            Self::Stream => "stream",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRecordDetail {
    pub id: String,
    pub provider: String,
    pub source_mode: String,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub model: Option<String>,
    pub is_stream: bool,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub reasoning_tokens: i64,
    pub ttft_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub cwd: Option<String>,
    pub entrypoint: Option<String>,
    pub request_summary_json: Option<String>,
    pub response_summary_json: Option<String>,
    pub error_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestFilterInput {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub is_stream: Option<bool>,
    pub started_after: Option<String>,
    pub started_before: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedRequestRecords {
    pub records: Vec<RequestRecordListItem>,
    pub total: i64,
    pub total_input_tokens: i64,
    pub total_cached_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_reasoning_tokens: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIChatCompletionRequest {
    pub model: String,
    pub messages: Vec<OpenAIChatMessage>,
    pub tools: Option<Vec<serde_json::Value>>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIChatMessage {
    pub role: String,
    pub content: serde_json::Value,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChatChoice>,
    pub usage: OpenAIUsage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIChatChoice {
    pub index: i64,
    pub message: OpenAIChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIResponsesRequest {
    pub model: String,
    pub input: serde_json::Value,
    pub tools: Option<Vec<serde_json::Value>>,
    pub temperature: Option<f64>,
    pub max_output_tokens: Option<i64>,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIResponsesResponse {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub model: String,
    pub output: Vec<serde_json::Value>,
    pub usage: OpenAIUsage,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub system: Option<serde_json::Value>,
    pub tools: Option<Vec<serde_json::Value>>,
    pub temperature: Option<f64>,
    pub max_tokens: i64,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnthropicMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnthropicMessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub role: String,
    pub model: String,
    pub content: Vec<serde_json::Value>,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnthropicUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatApiStatus {
    pub running: bool,
    pub listen_address: String,
    pub started_at: Option<String>,
    pub profiles_count: i64,
}
