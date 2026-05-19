use async_stream::stream;
use axum::{
    extract::State,
    response::{sse::Event, Sse},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use futures::stream::Stream;
use reqwest::Client;
use serde_json::json;
use std::{net::SocketAddr, sync::Arc, time::Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::db;
use crate::models::{
    AnthropicMessagesRequest, AnthropicMessagesResponse, AnthropicUsage, CompatApiStatus,
    NormalizedRequest, OpenAIChatChoice, OpenAIChatChunkChoice, OpenAIChatCompletionChunk,
    OpenAIChatCompletionRequest, OpenAIChatCompletionResponse, OpenAIChatMessage,
    OpenAIResponsesRequest, OpenAIResponsesResponse, OpenAIUsage, ProviderProfileRecord,
    RequestRecordUpsertRecord,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpstreamProtocol {
    OpenAI,
    Anthropic,
}

#[derive(Clone)]
pub struct CompatApiState {
    pub app_handle: tauri::AppHandle,
    pub http_client: Client,
}

#[derive(Clone)]
pub struct CompatApiServer {
    pub state: CompatApiState,
    pub listen_address: String,
    pub running: Arc<Mutex<bool>>,
    pub started_at: Arc<Mutex<Option<String>>>,
}

impl CompatApiServer {
    pub fn new(app_handle: tauri::AppHandle, listen_address: String) -> Self {
        Self {
            state: CompatApiState {
                app_handle,
                http_client: Client::new(),
            },
            listen_address,
            running: Arc::new(Mutex::new(false)),
            started_at: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(&self) -> Result<(), String> {
        let mut running = self.running.lock().await;
        if *running {
            return Err("Compat API server is already running".to_string());
        }

        let addr: SocketAddr = self
            .listen_address
            .parse()
            .map_err(|e| format!("Invalid listen address: {}", e))?;

        let state = self.state.clone();
        let app = create_router(state);

        let running_clone = self.running.clone();
        let started_at_clone = self.started_at.clone();

        tokio::spawn(async move {
            *started_at_clone.lock().await = Some(Utc::now().to_rfc3339());
            *running_clone.lock().await = true;

            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("Failed to bind to {}: {}", addr, e);
                    *running_clone.lock().await = false;
                    return;
                }
            };

            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("Compat API server error: {}", e);
            }

            *running_clone.lock().await = false;
        });

        *running = true;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut running = self.running.lock().await;
        if !*running {
            return Err("Compat API server is not running".to_string());
        }
        *running = false;
        *self.started_at.lock().await = None;
        Ok(())
    }

    pub async fn get_status(&self) -> CompatApiStatus {
        let running = *self.running.lock().await;
        let started_at = self.started_at.lock().await.clone();
        let profiles_count = if let Ok(conn) = db::get_connection(&self.state.app_handle) {
            db::list_provider_profiles_from_conn(&conn)
                .map(|p| p.len() as i64)
                .unwrap_or(0)
        } else {
            0
        };

        CompatApiStatus {
            running,
            listen_address: self.listen_address.clone(),
            started_at,
            profiles_count,
        }
    }
}

fn create_router(state: CompatApiState) -> Router {
    Router::new()
        .route("/v1/responses", post(handle_responses))
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/messages", post(handle_messages))
        .with_state(state)
}

async fn handle_responses(
    State(state): State<CompatApiState>,
    Json(req): Json<OpenAIResponsesRequest>,
) -> Json<OpenAIResponsesResponse> {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let is_stream = req.stream.unwrap_or(false);

    let base_url = resolve_upstream_url(&state, &req.model, UpstreamProtocol::OpenAI)
        .unwrap_or_else(|| "https://api.openai.com".to_string());
    let api_key = resolve_api_key(&state, &req.model, UpstreamProtocol::OpenAI).unwrap_or_default();

    if is_stream {
        let response = OpenAIResponsesResponse {
            id: request_id.clone(),
            object: "response".to_string(),
            created_at: Utc::now().timestamp(),
            model: req.model.clone(),
            output: vec![
                json!({"type": "message", "content": [{"type": "text", "text": "Streaming not yet implemented for Responses API"}]}),
            ],
            usage: OpenAIUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        };
        return Json(response);
    }

    let mut request_body = serde_json::json!({
        "model": req.model,
        "input": req.input,
    });

    if let Some(ref tools) = req.tools {
        request_body["tools"] = json!(tools);
    }
    if let Some(temp) = req.temperature {
        request_body["temperature"] = json!(temp);
    }
    if let Some(max_tokens) = req.max_output_tokens {
        request_body["max_output_tokens"] = json!(max_tokens);
    }

    let response = state
        .http_client
        .post(format!("{}/v1/responses", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await;

    let duration_ms = start_time.elapsed().as_millis() as i64;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
                let usage = body.get("usage").cloned().unwrap_or(json!({}));
                let input_tokens = usage
                    .get("input_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i64;
                let output_tokens = usage
                    .get("output_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i64;

                let record = RequestRecordUpsertRecord {
                    id: request_id.clone(),
                    provider: "openai_compat".to_string(),
                    source_mode: "local_compat_api".to_string(),
                    session_id: None,
                    request_id: Some(request_id.clone()),
                    model: Some(req.model.clone()),
                    is_stream: false,
                    input_tokens,
                    output_tokens,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    ttft_ms: None,
                    duration_ms: Some(duration_ms),
                    status: "success".to_string(),
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                    response_summary_json: Some(serde_json::to_string(&body).unwrap_or_default()),
                    error_text: None,
                };

                if let Ok(conn) = db::get_connection(&state.app_handle) {
                    let _ = db::upsert_request_record(&conn, &record);
                    let _ = db::rebuild_daily_usage_for_provider(&conn, "openai_compat");
                }

                let response_obj = OpenAIResponsesResponse {
                    id: request_id,
                    object: "response".to_string(),
                    created_at: Utc::now().timestamp(),
                    model: req.model.clone(),
                    output: body
                        .get("output")
                        .cloned()
                        .unwrap_or(json!([]))
                        .as_array()
                        .cloned()
                        .unwrap_or_default(),
                    usage: OpenAIUsage {
                        prompt_tokens: input_tokens,
                        completion_tokens: output_tokens,
                        total_tokens: input_tokens + output_tokens,
                    },
                };
                Json(response_obj)
            } else {
                let error_text = resp.text().await.unwrap_or_default();
                let record = RequestRecordUpsertRecord {
                    id: request_id.clone(),
                    provider: "openai_compat".to_string(),
                    source_mode: "local_compat_api".to_string(),
                    session_id: None,
                    request_id: Some(request_id.clone()),
                    model: Some(req.model.clone()),
                    is_stream: false,
                    input_tokens: 0,
                    output_tokens: 0,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    ttft_ms: None,
                    duration_ms: Some(duration_ms),
                    status: format!("error_{}", status.as_u16()),
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                    response_summary_json: None,
                    error_text: Some(error_text),
                };

                if let Ok(conn) = db::get_connection(&state.app_handle) {
                    let _ = db::upsert_request_record(&conn, &record);
                }

                Json(OpenAIResponsesResponse {
                    id: request_id,
                    object: "response".to_string(),
                    created_at: Utc::now().timestamp(),
                    model: req.model.clone(),
                    output: vec![
                        json!({"type": "error", "content": [{"type": "text", "text": "Upstream request failed"}]}),
                    ],
                    usage: OpenAIUsage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                })
            }
        }
        Err(e) => {
            let record = RequestRecordUpsertRecord {
                id: request_id.clone(),
                provider: "openai_compat".to_string(),
                source_mode: "local_compat_api".to_string(),
                session_id: None,
                request_id: Some(request_id.clone()),
                model: Some(req.model.clone()),
                is_stream: false,
                input_tokens: 0,
                output_tokens: 0,
                cached_input_tokens: 0,
                reasoning_tokens: 0,
                ttft_ms: None,
                duration_ms: Some(duration_ms),
                status: "error_network".to_string(),
                started_at: Utc::now().to_rfc3339(),
                finished_at: Some(Utc::now().to_rfc3339()),
                request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                response_summary_json: None,
                error_text: Some(e.to_string()),
            };

            if let Ok(conn) = db::get_connection(&state.app_handle) {
                let _ = db::upsert_request_record(&conn, &record);
            }

            Json(OpenAIResponsesResponse {
                id: request_id,
                object: "response".to_string(),
                created_at: Utc::now().timestamp(),
                model: req.model.clone(),
                output: vec![
                    json!({"type": "error", "content": [{"type": "text", "text": "Network error"}]}),
                ],
                usage: OpenAIUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
            })
        }
    }
}

async fn handle_chat_completions(
    State(state): State<CompatApiState>,
    Json(req): Json<OpenAIChatCompletionRequest>,
) -> Json<OpenAIChatCompletionResponse> {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let is_stream = req.stream.unwrap_or(false);

    let base_url = resolve_upstream_url(&state, &req.model, UpstreamProtocol::OpenAI)
        .unwrap_or_else(|| "https://api.openai.com".to_string());
    let api_key = resolve_api_key(&state, &req.model, UpstreamProtocol::OpenAI).unwrap_or_default();

    if is_stream {
        return Json(OpenAIChatCompletionResponse {
            id: request_id,
            object: "chat.completion".to_string(),
            created: Utc::now().timestamp(),
            model: req.model.clone(),
            choices: vec![OpenAIChatChoice {
                index: 0,
                message: OpenAIChatMessage {
                    role: "assistant".to_string(),
                    content: json!("Streaming mode - use SSE endpoint directly"),
                },
                finish_reason: "stop".to_string(),
            }],
            usage: OpenAIUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        });
    }

    let mut request_body = serde_json::json!({
        "model": req.model,
        "messages": req.messages,
    });

    if let Some(ref tools) = req.tools {
        request_body["tools"] = json!(tools);
    }
    if let Some(temp) = req.temperature {
        request_body["temperature"] = json!(temp);
    }
    if let Some(max_tokens) = req.max_tokens {
        request_body["max_tokens"] = json!(max_tokens);
    }

    let response = state
        .http_client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await;

    let duration_ms = start_time.elapsed().as_millis() as i64;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
                let usage = body.get("usage").cloned().unwrap_or(json!({}));
                let input_tokens = usage
                    .get("prompt_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i64;
                let output_tokens = usage
                    .get("completion_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i64;

                let choices: Vec<OpenAIChatChoice> = body
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .map(|choices| {
                        choices
                            .iter()
                            .enumerate()
                            .map(|(i, choice)| {
                                let message = choice.get("message").cloned().unwrap_or(json!({}));
                                let role = message
                                    .get("role")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("assistant")
                                    .to_string();
                                let content = message.get("content").cloned().unwrap_or(json!(""));
                                let finish_reason = choice
                                    .get("finish_reason")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("stop")
                                    .to_string();
                                OpenAIChatChoice {
                                    index: i as i64,
                                    message: OpenAIChatMessage { role, content },
                                    finish_reason,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let record = RequestRecordUpsertRecord {
                    id: request_id.clone(),
                    provider: "openai_compat".to_string(),
                    source_mode: "local_compat_api".to_string(),
                    session_id: None,
                    request_id: Some(request_id.clone()),
                    model: Some(req.model.clone()),
                    is_stream: false,
                    input_tokens,
                    output_tokens,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    ttft_ms: None,
                    duration_ms: Some(duration_ms),
                    status: "success".to_string(),
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                    response_summary_json: Some(serde_json::to_string(&body).unwrap_or_default()),
                    error_text: None,
                };

                if let Ok(conn) = db::get_connection(&state.app_handle) {
                    let _ = db::upsert_request_record(&conn, &record);
                    let _ = db::rebuild_daily_usage_for_provider(&conn, "openai_compat");
                }

                Json(OpenAIChatCompletionResponse {
                    id: request_id,
                    object: "chat.completion".to_string(),
                    created: Utc::now().timestamp(),
                    model: req.model.clone(),
                    choices,
                    usage: OpenAIUsage {
                        prompt_tokens: input_tokens,
                        completion_tokens: output_tokens,
                        total_tokens: input_tokens + output_tokens,
                    },
                })
            } else {
                let error_text = resp.text().await.unwrap_or_default();
                let record = RequestRecordUpsertRecord {
                    id: request_id.clone(),
                    provider: "openai_compat".to_string(),
                    source_mode: "local_compat_api".to_string(),
                    session_id: None,
                    request_id: Some(request_id.clone()),
                    model: Some(req.model.clone()),
                    is_stream: false,
                    input_tokens: 0,
                    output_tokens: 0,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    ttft_ms: None,
                    duration_ms: Some(duration_ms),
                    status: format!("error_{}", status.as_u16()),
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                    response_summary_json: None,
                    error_text: Some(error_text),
                };

                if let Ok(conn) = db::get_connection(&state.app_handle) {
                    let _ = db::upsert_request_record(&conn, &record);
                }

                Json(OpenAIChatCompletionResponse {
                    id: request_id,
                    object: "chat.completion".to_string(),
                    created: Utc::now().timestamp(),
                    model: req.model.clone(),
                    choices: vec![OpenAIChatChoice {
                        index: 0,
                        message: OpenAIChatMessage {
                            role: "assistant".to_string(),
                            content: json!("Upstream request failed"),
                        },
                        finish_reason: "error".to_string(),
                    }],
                    usage: OpenAIUsage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                })
            }
        }
        Err(e) => {
            let record = RequestRecordUpsertRecord {
                id: request_id.clone(),
                provider: "openai_compat".to_string(),
                source_mode: "local_compat_api".to_string(),
                session_id: None,
                request_id: Some(request_id.clone()),
                model: Some(req.model.clone()),
                is_stream: false,
                input_tokens: 0,
                output_tokens: 0,
                cached_input_tokens: 0,
                reasoning_tokens: 0,
                ttft_ms: None,
                duration_ms: Some(duration_ms),
                status: "error_network".to_string(),
                started_at: Utc::now().to_rfc3339(),
                finished_at: Some(Utc::now().to_rfc3339()),
                request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                response_summary_json: None,
                error_text: Some(e.to_string()),
            };

            if let Ok(conn) = db::get_connection(&state.app_handle) {
                let _ = db::upsert_request_record(&conn, &record);
            }

            Json(OpenAIChatCompletionResponse {
                id: request_id,
                object: "chat.completion".to_string(),
                created: Utc::now().timestamp(),
                model: req.model.clone(),
                choices: vec![OpenAIChatChoice {
                    index: 0,
                    message: OpenAIChatMessage {
                        role: "assistant".to_string(),
                        content: json!("Network error"),
                    },
                    finish_reason: "error".to_string(),
                }],
                usage: OpenAIUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
            })
        }
    }
}

async fn handle_messages(
    State(state): State<CompatApiState>,
    Json(req): Json<AnthropicMessagesRequest>,
) -> Json<AnthropicMessagesResponse> {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let is_stream = req.stream.unwrap_or(false);

    let base_url = resolve_upstream_url(&state, &req.model, UpstreamProtocol::Anthropic)
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    let api_key =
        resolve_api_key(&state, &req.model, UpstreamProtocol::Anthropic).unwrap_or_default();

    if is_stream {
        return Json(AnthropicMessagesResponse {
            id: request_id,
            type_field: "message".to_string(),
            role: "assistant".to_string(),
            model: req.model.clone(),
            content: vec![
                json!({"type": "text", "text": "Streaming mode - use SSE endpoint directly"}),
            ],
            usage: AnthropicUsage {
                input_tokens: 0,
                output_tokens: 0,
            },
        });
    }

    let mut request_body = serde_json::json!({
        "model": req.model,
        "messages": req.messages,
        "max_tokens": req.max_tokens,
    });

    if let Some(ref system) = req.system {
        request_body["system"] = system.clone();
    }
    if let Some(ref tools) = req.tools {
        request_body["tools"] = json!(tools);
    }
    if let Some(temp) = req.temperature {
        request_body["temperature"] = json!(temp);
    }

    let response = state
        .http_client
        .post(format!("{}/v1/messages", base_url))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await;

    let duration_ms = start_time.elapsed().as_millis() as i64;

    match response {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
                let usage = body.get("usage").cloned().unwrap_or(json!({}));
                let input_tokens = usage
                    .get("input_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i64;
                let output_tokens = usage
                    .get("output_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i64;

                let content: Vec<serde_json::Value> = body
                    .get("content")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();

                let record = RequestRecordUpsertRecord {
                    id: request_id.clone(),
                    provider: "anthropic_compat".to_string(),
                    source_mode: "local_compat_api".to_string(),
                    session_id: None,
                    request_id: Some(request_id.clone()),
                    model: Some(req.model.clone()),
                    is_stream: false,
                    input_tokens,
                    output_tokens,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    ttft_ms: None,
                    duration_ms: Some(duration_ms),
                    status: "success".to_string(),
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                    response_summary_json: Some(serde_json::to_string(&body).unwrap_or_default()),
                    error_text: None,
                };

                if let Ok(conn) = db::get_connection(&state.app_handle) {
                    let _ = db::upsert_request_record(&conn, &record);
                    let _ = db::rebuild_daily_usage_for_provider(&conn, "anthropic_compat");
                }

                Json(AnthropicMessagesResponse {
                    id: request_id,
                    type_field: "message".to_string(),
                    role: "assistant".to_string(),
                    model: req.model.clone(),
                    content,
                    usage: AnthropicUsage {
                        input_tokens,
                        output_tokens,
                    },
                })
            } else {
                let error_text = resp.text().await.unwrap_or_default();
                let record = RequestRecordUpsertRecord {
                    id: request_id.clone(),
                    provider: "anthropic_compat".to_string(),
                    source_mode: "local_compat_api".to_string(),
                    session_id: None,
                    request_id: Some(request_id.clone()),
                    model: Some(req.model.clone()),
                    is_stream: false,
                    input_tokens: 0,
                    output_tokens: 0,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    ttft_ms: None,
                    duration_ms: Some(duration_ms),
                    status: format!("error_{}", status.as_u16()),
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                    response_summary_json: None,
                    error_text: Some(error_text),
                };

                if let Ok(conn) = db::get_connection(&state.app_handle) {
                    let _ = db::upsert_request_record(&conn, &record);
                }

                Json(AnthropicMessagesResponse {
                    id: request_id,
                    type_field: "message".to_string(),
                    role: "assistant".to_string(),
                    model: req.model.clone(),
                    content: vec![json!({"type": "text", "text": "Upstream request failed"})],
                    usage: AnthropicUsage {
                        input_tokens: 0,
                        output_tokens: 0,
                    },
                })
            }
        }
        Err(e) => {
            let record = RequestRecordUpsertRecord {
                id: request_id.clone(),
                provider: "anthropic_compat".to_string(),
                source_mode: "local_compat_api".to_string(),
                session_id: None,
                request_id: Some(request_id.clone()),
                model: Some(req.model.clone()),
                is_stream: false,
                input_tokens: 0,
                output_tokens: 0,
                cached_input_tokens: 0,
                reasoning_tokens: 0,
                ttft_ms: None,
                duration_ms: Some(duration_ms),
                status: "error_network".to_string(),
                started_at: Utc::now().to_rfc3339(),
                finished_at: Some(Utc::now().to_rfc3339()),
                request_summary_json: Some(serde_json::to_string(&req).unwrap_or_default()),
                response_summary_json: None,
                error_text: Some(e.to_string()),
            };

            if let Ok(conn) = db::get_connection(&state.app_handle) {
                let _ = db::upsert_request_record(&conn, &record);
            }

            Json(AnthropicMessagesResponse {
                id: request_id,
                type_field: "message".to_string(),
                role: "assistant".to_string(),
                model: req.model.clone(),
                content: vec![json!({"type": "text", "text": "Network error"})],
                usage: AnthropicUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            })
        }
    }
}

fn resolve_upstream_url(
    state: &CompatApiState,
    model: &str,
    protocol: UpstreamProtocol,
) -> Option<String> {
    let profile = find_matching_profile(state, model, protocol)?;
    profile
        .base_url
        .or_else(|| default_base_url(&profile.api_format, protocol))
}

fn resolve_api_key(
    state: &CompatApiState,
    model: &str,
    protocol: UpstreamProtocol,
) -> Option<String> {
    let profile = find_matching_profile(state, model, protocol)?;
    profile
        .api_key_env
        .as_deref()
        .and_then(|env_key| std::env::var(env_key).ok())
}

fn find_matching_profile(
    state: &CompatApiState,
    model: &str,
    protocol: UpstreamProtocol,
) -> Option<ProviderProfileRecord> {
    let conn = db::get_connection(&state.app_handle).ok()?;
    let profiles = db::list_provider_profiles_from_conn(&conn).ok()?;

    profiles
        .into_iter()
        .filter_map(|profile| {
            profile_match_score(&profile, model, protocol).map(|score| (score, profile))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, profile)| profile)
}

fn profile_match_score(
    profile: &ProviderProfileRecord,
    model: &str,
    protocol: UpstreamProtocol,
) -> Option<(u8, usize)> {
    if !profile.enabled || !supports_protocol(profile.api_format.as_str(), protocol) {
        return None;
    }

    let exact_models = profile_exact_models(profile);
    if exact_models.iter().any(|candidate| candidate == model) {
        return Some((3, model.len()));
    }

    let matching_prefix_length = profile_model_prefixes(profile)
        .into_iter()
        .filter(|prefix| model.starts_with(prefix))
        .map(|prefix| prefix.len())
        .max();

    if let Some(prefix_length) = matching_prefix_length {
        return Some((2, prefix_length));
    }

    if exact_models.is_empty() {
        let configured_prefixes = profile_model_prefixes(profile);
        if configured_prefixes.is_empty() {
            return default_protocol_match(profile.api_format.as_str(), model, protocol)
                .then_some((1, 0));
        }
    }

    None
}

fn supports_protocol(api_format: &str, protocol: UpstreamProtocol) -> bool {
    match protocol {
        UpstreamProtocol::OpenAI => matches!(api_format, "openai" | "custom"),
        UpstreamProtocol::Anthropic => api_format == "anthropic",
    }
}

fn default_protocol_match(api_format: &str, model: &str, protocol: UpstreamProtocol) -> bool {
    match (protocol, api_format) {
        (UpstreamProtocol::OpenAI, "openai") => {
            model.starts_with("gpt-")
                || model.starts_with("o1")
                || model.starts_with("o3")
                || model.starts_with("o4")
        }
        (UpstreamProtocol::Anthropic, "anthropic") => model.starts_with("claude-"),
        _ => false,
    }
}

fn default_base_url(api_format: &str, protocol: UpstreamProtocol) -> Option<String> {
    match (protocol, api_format) {
        (UpstreamProtocol::OpenAI, "openai") => Some("https://api.openai.com".to_string()),
        (UpstreamProtocol::Anthropic, "anthropic") => Some("https://api.anthropic.com".to_string()),
        _ => None,
    }
}

fn profile_model_prefixes(profile: &ProviderProfileRecord) -> Vec<String> {
    let Some(extra) = parse_profile_extra(profile) else {
        return Vec::new();
    };

    let mut prefixes = extract_string_list(extra.get("model_prefixes"));
    if prefixes.is_empty() {
        prefixes = extract_string_list(extra.get("modelPrefixes"));
    }

    prefixes
}

fn profile_exact_models(profile: &ProviderProfileRecord) -> Vec<String> {
    let Some(extra) = parse_profile_extra(profile) else {
        return Vec::new();
    };

    extract_string_list(extra.get("models"))
}

fn parse_profile_extra(profile: &ProviderProfileRecord) -> Option<serde_json::Value> {
    let extra = profile.extra_json.as_deref()?;
    serde_json::from_str::<serde_json::Value>(extra).ok()
}

fn extract_string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .filter_map(|value| value.as_str().map(|value| value.trim().to_string()))
            .filter(|value| !value.is_empty())
            .collect(),
        Some(serde_json::Value::String(value)) => value
            .split([',', '\n'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

pub async fn create_sse_stream(
    state: CompatApiState,
    normalized_req: NormalizedRequest,
    request_id: String,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let stream = stream! {
        let start_time = Instant::now();
        let mut ttft_ms: Option<i64> = None;
        let mut first_chunk_received = false;

        let base_url = resolve_upstream_url(&state, &normalized_req.model, UpstreamProtocol::OpenAI)
            .unwrap_or_else(|| "https://api.openai.com".to_string());
        let api_key =
            resolve_api_key(&state, &normalized_req.model, UpstreamProtocol::OpenAI).unwrap_or_default();

        let request_body = serde_json::json!({
            "model": normalized_req.model,
            "messages": normalized_req.messages_or_input,
            "stream": true,
        });

        let response = state
            .http_client
            .post(format!("{}/v1/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let mut stream = resp.bytes_stream();
                use futures::StreamExt;
                while let Some(chunk) = stream.next().await {
                    if !first_chunk_received {
                        ttft_ms = Some(start_time.elapsed().as_millis() as i64);
                        first_chunk_received = true;
                    }

                    if let Ok(bytes) = chunk {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let data = &line[6..];
                                    if data == "[DONE]" {
                                        yield Ok(Event::default().data("[DONE]"));
                                    } else if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(data) {
                                        let chunk_obj = OpenAIChatCompletionChunk {
                                            id: request_id.clone(),
                                            object: "chat.completion.chunk".to_string(),
                                            created: Utc::now().timestamp(),
                                            model: normalized_req.model.clone(),
                                            choices: vec![OpenAIChatChunkChoice {
                                                index: 0,
                                                delta: OpenAIChatMessage {
                                                    role: "assistant".to_string(),
                                                    content: json_value.clone(),
                                                },
                                                finish_reason: json_value.get("choices")
                                                    .and_then(|c| c.as_array())
                                                    .and_then(|c| c.first())
                                                    .and_then(|c| c.get("finish_reason"))
                                                    .and_then(|f| f.as_str())
                                                    .map(String::from),
                                            }],
                                        };
                                        yield Ok(Event::default().data(serde_json::to_string(&chunk_obj).unwrap_or_default()));
                                    }
                                }
                            }
                        }
                    }
                }

                let duration_ms = start_time.elapsed().as_millis() as i64;

                let record = RequestRecordUpsertRecord {
                    id: request_id.clone(),
                    provider: "openai_compat".to_string(),
                    source_mode: "local_compat_api".to_string(),
                    session_id: None,
                    request_id: Some(request_id.clone()),
                    model: Some(normalized_req.model.clone()),
                    is_stream: true,
                    input_tokens: 0,
                    output_tokens: 0,
                    cached_input_tokens: 0,
                    reasoning_tokens: 0,
                    ttft_ms,
                    duration_ms: Some(duration_ms),
                    status: "success".to_string(),
                    started_at: Utc::now().to_rfc3339(),
                    finished_at: Some(Utc::now().to_rfc3339()),
                    request_summary_json: Some(serde_json::to_string(&normalized_req).unwrap_or_default()),
                    response_summary_json: None,
                    error_text: None,
                };

                if let Ok(conn) = db::get_connection(&state.app_handle) {
                    let _ = db::upsert_request_record(&conn, &record);
                    let _ = db::rebuild_daily_usage_for_provider(&conn, "openai_compat");
                }
            }
            Err(_) => {
                yield Ok(Event::default().data("error: Network error"));
            }
        }
    };

    Sse::new(stream)
}
