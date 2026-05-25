use async_stream::stream;
use axum::{
    extract::State,
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde_json::json;
use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Instant};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use crate::db;
use crate::models::{
    AnthropicMessage, AnthropicMessagesRequest, AnthropicMessagesResponse, AnthropicUsage,
    CompatApiStatus, OpenAIChatChoice, OpenAIChatCompletionRequest, OpenAIChatCompletionResponse,
    OpenAIChatMessage, OpenAIResponsesRequest, OpenAIResponsesResponse, OpenAIUsage,
    ProviderProfileRecord, RequestRecordUpsertRecord, RequestType,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpstreamProtocol {
    OpenAI,
    Anthropic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestProtocol {
    OpenAI,
    Anthropic,
}

#[derive(Clone, Copy)]
enum StreamTransform {
    OpenAIChatToOpenAIChat,
    AnthropicToOpenAIChat,
    AnthropicToAnthropic,
    OpenAIChatToAnthropic,
    OpenAIResponses,
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
    shutdown_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
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
            shutdown_tx: Arc::new(Mutex::new(None)),
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

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;
        let app = create_router(self.state.clone());
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        *self.shutdown_tx.lock().await = Some(shutdown_tx);
        *self.started_at.lock().await = Some(Utc::now().to_rfc3339());
        *running = true;

        let running_clone = self.running.clone();
        let started_at_clone = self.started_at.clone();
        let shutdown_tx_clone = self.shutdown_tx.clone();

        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });

            if let Err(e) = server.await {
                eprintln!("Compat API server error: {}", e);
            }

            *running_clone.lock().await = false;
            *started_at_clone.lock().await = None;
            *shutdown_tx_clone.lock().await = None;
        });
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut running = self.running.lock().await;
        if !*running {
            return Err("Compat API server is not running".to_string());
        }
        if let Some(shutdown_tx) = self.shutdown_tx.lock().await.take() {
            let _ = shutdown_tx.send(());
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
        .route("/compat/health", get(handle_compat_health))
        .route("/v1/models", get(handle_models))
        .route("/v1/responses", post(handle_responses))
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/messages", post(handle_messages))
        .with_state(state)
}

async fn handle_compat_health(State(state): State<CompatApiState>) -> Json<serde_json::Value> {
    let profiles_count = db::get_connection(&state.app_handle)
        .ok()
        .and_then(|conn| db::list_provider_profiles_from_conn(&conn).ok())
        .map(|profiles| {
            profiles
                .into_iter()
                .filter(|profile| profile.enabled)
                .count()
        })
        .unwrap_or(0);

    Json(json!({
        "status": "ok",
        "endpoints": [
            "/v1/models",
            "/v1/responses",
            "/v1/chat/completions",
            "/v1/messages"
        ],
        "profilesEnabled": profiles_count
    }))
}

async fn handle_models(State(state): State<CompatApiState>) -> Json<serde_json::Value> {
    let models = db::get_connection(&state.app_handle)
        .ok()
        .and_then(|conn| db::list_provider_profiles_from_conn(&conn).ok())
        .map(|profiles| {
            profiles
                .into_iter()
                .filter(|profile| profile.enabled)
                .flat_map(|profile| {
                    let exact_models = profile_exact_models(&profile);
                    if exact_models.is_empty() {
                        vec![json!({
                            "id": profile.provider_key,
                            "object": "model",
                            "created": 0,
                            "owned_by": profile.display_name,
                            "api_format": profile.api_format,
                        })]
                    } else {
                        exact_models
                            .into_iter()
                            .map(|model| {
                                json!({
                                    "id": model,
                                    "object": "model",
                                    "created": 0,
                                    "owned_by": profile.display_name,
                                    "api_format": profile.api_format,
                                })
                            })
                            .collect()
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Json(json!({
        "object": "list",
        "data": models
    }))
}

async fn handle_responses(
    State(state): State<CompatApiState>,
    Json(req): Json<OpenAIResponsesRequest>,
) -> Response {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let is_stream = req.stream.unwrap_or(false);

    let base_url = resolve_upstream_url(&state, &req.model, UpstreamProtocol::OpenAI)
        .unwrap_or_else(|| "https://api.openai.com".to_string());
    let api_key = resolve_api_key(&state, &req.model, UpstreamProtocol::OpenAI).unwrap_or_default();

    if is_stream {
        let mut request_body = serde_json::json!({
            "model": req.model,
            "input": req.input,
            "stream": true,
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

        return create_compat_sse_stream(
            state,
            request_id,
            "openai_compat",
            req.model.clone(),
            request_body,
            format!("{}/v1/responses", base_url),
            vec![
                ("Authorization".to_string(), format!("Bearer {}", api_key)),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            StreamTransform::OpenAIResponses,
            serde_json::to_string(&req).ok(),
        )
        .into_response();
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
                Json(response_obj).into_response()
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
                .into_response()
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
            .into_response()
        }
    }
}

async fn handle_chat_completions(
    State(state): State<CompatApiState>,
    Json(req): Json<OpenAIChatCompletionRequest>,
) -> Response {
    let start_time = Instant::now();
    let request_id = Uuid::new_v4().to_string();
    let is_stream = req.stream.unwrap_or(false);

    let upstream_protocol = resolve_upstream_protocol(&state, &req.model, RequestProtocol::OpenAI)
        .unwrap_or(UpstreamProtocol::OpenAI);

    if upstream_protocol == UpstreamProtocol::Anthropic {
        return handle_chat_completions_via_anthropic(state, req, request_id, start_time)
            .await
            .into_response();
    }

    let base_url = resolve_upstream_url(&state, &req.model, UpstreamProtocol::OpenAI)
        .unwrap_or_else(|| "https://api.openai.com".to_string());
    let api_key = resolve_api_key(&state, &req.model, UpstreamProtocol::OpenAI).unwrap_or_default();

    if is_stream {
        let mut request_body = serde_json::json!({
            "model": req.model,
            "messages": req.messages,
            "stream": true,
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

        return create_compat_sse_stream(
            state,
            request_id,
            "openai_compat",
            req.model.clone(),
            request_body,
            format!("{}/v1/chat/completions", base_url),
            vec![
                ("Authorization".to_string(), format!("Bearer {}", api_key)),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            StreamTransform::OpenAIChatToOpenAIChat,
            serde_json::to_string(&req).ok(),
        )
        .into_response();
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
                                    message: OpenAIChatMessage {
                                        role,
                                        content,
                                        extra: message
                                            .as_object()
                                            .map(|object| {
                                                object
                                                    .iter()
                                                    .filter(|(key, _)| {
                                                        key.as_str() != "role"
                                                            && key.as_str() != "content"
                                                    })
                                                    .map(|(key, value)| {
                                                        (key.clone(), value.clone())
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default(),
                                    },
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
                .into_response()
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
                            extra: serde_json::Map::new(),
                        },
                        finish_reason: "error".to_string(),
                    }],
                    usage: OpenAIUsage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                })
                .into_response()
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
                        extra: serde_json::Map::new(),
                    },
                    finish_reason: "error".to_string(),
                }],
                usage: OpenAIUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                },
            })
            .into_response()
        }
    }
}

async fn handle_messages(
    State(state): State<CompatApiState>,
    Json(req): Json<AnthropicMessagesRequest>,
) -> Response {
    let start_time = Instant::now();
    let started_at = Utc::now().to_rfc3339();
    let request_id = Uuid::new_v4().to_string();
    let is_stream = req.stream.unwrap_or(false);

    let upstream_protocol =
        resolve_upstream_protocol(&state, &req.model, RequestProtocol::Anthropic)
            .unwrap_or(UpstreamProtocol::Anthropic);

    if upstream_protocol == UpstreamProtocol::OpenAI {
        return handle_messages_via_openai(state, req, request_id, start_time)
            .await
            .into_response();
    }

    let base_url = resolve_upstream_url(&state, &req.model, UpstreamProtocol::Anthropic)
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    let api_key =
        resolve_api_key(&state, &req.model, UpstreamProtocol::Anthropic).unwrap_or_default();

    if is_stream {
        let mut request_body = serde_json::json!({
            "model": req.model,
            "messages": req.messages,
            "max_tokens": req.max_tokens,
            "stream": true,
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

        return create_compat_sse_stream(
            state,
            request_id,
            "anthropic_compat",
            req.model.clone(),
            request_body,
            format!("{}/v1/messages", base_url),
            vec![
                ("x-api-key".to_string(), api_key),
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            StreamTransform::AnthropicToAnthropic,
            serde_json::to_string(&req).ok(),
        )
        .into_response();
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
                    started_at: started_at.clone(),
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
                .into_response()
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
                    started_at: started_at.clone(),
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
                .into_response()
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
                started_at: started_at.clone(),
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
            .into_response()
        }
    }
}

async fn handle_messages_via_openai(
    state: CompatApiState,
    req: AnthropicMessagesRequest,
    request_id: String,
    start_time: Instant,
) -> Response {
    let base_url = resolve_upstream_url(&state, &req.model, UpstreamProtocol::OpenAI)
        .unwrap_or_else(|| "https://api.openai.com".to_string());
    let api_key = resolve_api_key(&state, &req.model, UpstreamProtocol::OpenAI).unwrap_or_default();

    let mut request_body = serde_json::json!({
        "model": req.model,
        "messages": anthropic_messages_to_openai(&req.system, &req.messages),
        "max_tokens": req.max_tokens,
        "stream": req.stream.unwrap_or(false),
    });

    if let Some(ref tools) = req.tools {
        request_body["tools"] = json!(anthropic_tools_to_openai(tools));
    }
    if let Some(temp) = req.temperature {
        request_body["temperature"] = json!(temp);
    }

    if req.stream.unwrap_or(false) {
        return create_compat_sse_stream(
            state,
            request_id,
            "anthropic_compat",
            req.model.clone(),
            request_body,
            format!("{}/v1/chat/completions", base_url),
            vec![
                ("Authorization".to_string(), format!("Bearer {}", api_key)),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            StreamTransform::OpenAIChatToAnthropic,
            serde_json::to_string(&req).ok(),
        )
        .into_response();
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
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
            let usage = body.get("usage").cloned().unwrap_or(json!({}));
            let input_tokens = usage
                .get("prompt_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let output_tokens = usage
                .get("completion_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let content = openai_chat_body_to_anthropic_content(&body);

            record_compat_request(
                &state,
                &request_id,
                "anthropic_compat",
                &req.model,
                false,
                input_tokens,
                output_tokens,
                duration_ms,
                "success",
                Some(&req),
                Some(&body),
                None,
            );

            Json(AnthropicMessagesResponse {
                id: request_id,
                type_field: "message".to_string(),
                role: "assistant".to_string(),
                model: req.model,
                content,
                usage: AnthropicUsage {
                    input_tokens,
                    output_tokens,
                },
            })
            .into_response()
        }
        Ok(resp) => {
            let status = resp.status();
            let error_text = resp.text().await.unwrap_or_default();
            record_compat_request(
                &state,
                &request_id,
                "anthropic_compat",
                &req.model,
                false,
                0,
                0,
                duration_ms,
                &format!("error_{}", status.as_u16()),
                Some(&req),
                None::<&serde_json::Value>,
                Some(error_text),
            );
            Json(AnthropicMessagesResponse {
                id: request_id,
                type_field: "message".to_string(),
                role: "assistant".to_string(),
                model: req.model,
                content: vec![json!({"type": "text", "text": "OpenAI upstream request failed"})],
                usage: AnthropicUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            })
            .into_response()
        }
        Err(error) => {
            record_compat_request(
                &state,
                &request_id,
                "anthropic_compat",
                &req.model,
                false,
                0,
                0,
                duration_ms,
                "error_network",
                Some(&req),
                None::<&serde_json::Value>,
                Some(error.to_string()),
            );
            Json(AnthropicMessagesResponse {
                id: request_id,
                type_field: "message".to_string(),
                role: "assistant".to_string(),
                model: req.model,
                content: vec![json!({"type": "text", "text": "Network error"})],
                usage: AnthropicUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            })
            .into_response()
        }
    }
}

async fn handle_chat_completions_via_anthropic(
    state: CompatApiState,
    req: OpenAIChatCompletionRequest,
    request_id: String,
    start_time: Instant,
) -> Response {
    let base_url = resolve_upstream_url(&state, &req.model, UpstreamProtocol::Anthropic)
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    let api_key =
        resolve_api_key(&state, &req.model, UpstreamProtocol::Anthropic).unwrap_or_default();
    let (system, messages) = openai_messages_to_anthropic(&req.messages);

    let mut request_body = serde_json::json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": req.max_tokens.unwrap_or(4096),
        "stream": req.stream.unwrap_or(false),
    });
    if let Some(system) = system {
        request_body["system"] = system;
    }
    if let Some(ref tools) = req.tools {
        request_body["tools"] = json!(openai_tools_to_anthropic(tools));
    }
    if let Some(temp) = req.temperature {
        request_body["temperature"] = json!(temp);
    }

    if req.stream.unwrap_or(false) {
        return create_compat_sse_stream(
            state,
            request_id,
            "openai_compat",
            req.model.clone(),
            request_body,
            format!("{}/v1/messages", base_url),
            vec![
                ("x-api-key".to_string(), api_key),
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            StreamTransform::AnthropicToOpenAIChat,
            serde_json::to_string(&req).ok(),
        )
        .into_response();
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
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
            let usage = body.get("usage").cloned().unwrap_or(json!({}));
            let input_tokens = usage
                .get("input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let output_tokens = usage
                .get("output_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let message = anthropic_body_to_openai_message(&body);

            record_compat_request(
                &state,
                &request_id,
                "openai_compat",
                &req.model,
                false,
                input_tokens,
                output_tokens,
                duration_ms,
                "success",
                Some(&req),
                Some(&body),
                None,
            );

            Json(OpenAIChatCompletionResponse {
                id: request_id,
                object: "chat.completion".to_string(),
                created: Utc::now().timestamp(),
                model: req.model,
                choices: vec![OpenAIChatChoice {
                    index: 0,
                    message,
                    finish_reason: anthropic_stop_reason_to_openai(&body),
                }],
                usage: OpenAIUsage {
                    prompt_tokens: input_tokens,
                    completion_tokens: output_tokens,
                    total_tokens: input_tokens + output_tokens,
                },
            })
            .into_response()
        }
        Ok(resp) => {
            let status = resp.status();
            let error_text = resp.text().await.unwrap_or_default();
            record_compat_request(
                &state,
                &request_id,
                "openai_compat",
                &req.model,
                false,
                0,
                0,
                duration_ms,
                &format!("error_{}", status.as_u16()),
                Some(&req),
                None::<&serde_json::Value>,
                Some(error_text),
            );
            openai_error_response(request_id, req.model, "Anthropic upstream request failed")
                .into_response()
        }
        Err(error) => {
            record_compat_request(
                &state,
                &request_id,
                "openai_compat",
                &req.model,
                false,
                0,
                0,
                duration_ms,
                "error_network",
                Some(&req),
                None::<&serde_json::Value>,
                Some(error.to_string()),
            );
            openai_error_response(request_id, req.model, "Network error").into_response()
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

fn create_compat_sse_stream(
    state: CompatApiState,
    request_id: String,
    provider: &'static str,
    model: String,
    request_body: serde_json::Value,
    upstream_url: String,
    headers: Vec<(String, String)>,
    transform: StreamTransform,
    request_summary_json: Option<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = stream! {
        let started_at = Utc::now().to_rfc3339();
        let start_time = Instant::now();
        let mut ttft_ms: Option<i64> = None;
        let mut input_tokens = 0_i64;
        let mut output_tokens = 0_i64;
        let mut status = "success".to_string();
        let mut error_text: Option<String> = None;
        let mut builder = state.http_client.post(upstream_url).json(&request_body);

        for (name, value) in headers {
            builder = builder.header(name, value);
        }

        if matches!(transform, StreamTransform::OpenAIChatToAnthropic) {
            for event in anthropic_stream_start_events(&request_id, &model) {
                yield Ok(event);
            }
        }

        let response = builder.send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let mut event_name: Option<String> = None;
                let mut stream = resp.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            let Ok(text) = std::str::from_utf8(&bytes) else {
                                continue;
                            };

                            for line in text.lines() {
                                if let Some(name) = line.strip_prefix("event:") {
                                    event_name = Some(name.trim().to_string());
                                    continue;
                                }

                                let Some(data) = line.strip_prefix("data:") else {
                                    if line.trim().is_empty() {
                                        event_name = None;
                                    }
                                    continue;
                                };
                                let data = data.trim_start();

                                if data == "[DONE]" {
                                    if matches!(transform, StreamTransform::OpenAIChatToAnthropic) {
                                        yield Ok(Event::default().event("content_block_stop").data(json!({"type": "content_block_stop", "index": 0}).to_string()));
                                        yield Ok(Event::default().event("message_stop").data(json!({"type": "message_stop"}).to_string()));
                                    } else {
                                        yield Ok(Event::default().data("[DONE]"));
                                    }
                                    continue;
                                }

                                let parsed = serde_json::from_str::<serde_json::Value>(data).ok();
                                if ttft_ms.is_none() && parsed.as_ref().is_some_and(|value| stream_payload_has_visible_delta(value, transform)) {
                                    ttft_ms = Some(start_time.elapsed().as_millis() as i64);
                                }

                                if let Some(value) = parsed.as_ref() {
                                    let (input, output) = stream_usage_tokens(value);
                                    input_tokens = input_tokens.max(input);
                                    output_tokens = output_tokens.max(output);
                                }

                                for event in transform_stream_event(transform, event_name.as_deref(), data, parsed.as_ref(), &request_id, &model) {
                                    yield Ok(event);
                                }
                            }
                        }
                        Err(error) => {
                            status = "error_stream".to_string();
                            error_text = Some(error.to_string());
                            yield Ok(Event::default().data(format!("error: {}", error)));
                            break;
                        }
                    }
                }
            }
            Ok(resp) => {
                status = format!("error_{}", resp.status().as_u16());
                error_text = Some(resp.text().await.unwrap_or_default());
                yield Ok(Event::default().data(format!("error: {}", error_text.clone().unwrap_or_default())));
            }
            Err(error) => {
                status = "error_network".to_string();
                error_text = Some(error.to_string());
                yield Ok(Event::default().data(format!("error: {}", error)));
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as i64;
        record_stream_compat_request(
            &state,
            &request_id,
            provider,
            &model,
            input_tokens,
            output_tokens,
            ttft_ms,
            duration_ms,
            &status,
            request_summary_json,
            error_text,
            started_at,
        );
    };

    Sse::new(stream)
}

fn transform_stream_event(
    transform: StreamTransform,
    event_name: Option<&str>,
    data: &str,
    parsed: Option<&serde_json::Value>,
    request_id: &str,
    model: &str,
) -> Vec<Event> {
    match transform {
        StreamTransform::OpenAIChatToOpenAIChat | StreamTransform::OpenAIResponses => {
            vec![Event::default().data(data.to_string())]
        }
        StreamTransform::AnthropicToAnthropic => {
            let mut event = Event::default().data(data.to_string());
            if let Some(name) = event_name {
                event = event.event(name.to_string());
            }
            vec![event]
        }
        StreamTransform::AnthropicToOpenAIChat => parsed
            .map(|value| anthropic_event_to_openai_chat_events(value, request_id, model))
            .unwrap_or_default(),
        StreamTransform::OpenAIChatToAnthropic => parsed
            .map(|value| openai_chat_chunk_to_anthropic_events(value))
            .unwrap_or_default(),
    }
}

fn anthropic_stream_start_events(request_id: &str, model: &str) -> Vec<Event> {
    vec![
        Event::default().event("message_start").data(
            json!({
                "type": "message_start",
                "message": {
                    "id": request_id,
                    "type": "message",
                    "role": "assistant",
                    "model": model,
                    "content": [],
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": {"input_tokens": 0, "output_tokens": 0}
                }
            })
            .to_string(),
        ),
        Event::default().event("content_block_start").data(
            json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": {"type": "text", "text": ""}
            })
            .to_string(),
        ),
    ]
}

fn anthropic_event_to_openai_chat_events(
    value: &serde_json::Value,
    request_id: &str,
    model: &str,
) -> Vec<Event> {
    let event_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let created = Utc::now().timestamp();

    match event_type {
        "content_block_delta" => {
            let delta = value.get("delta").unwrap_or(&serde_json::Value::Null);
            match delta.get("type").and_then(|v| v.as_str()) {
                Some("text_delta") => vec![Event::default().data(json!({
                    "id": request_id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model,
                    "choices": [{
                        "index": 0,
                        "delta": {"content": delta.get("text").cloned().unwrap_or(json!(""))},
                        "finish_reason": null
                    }]
                }).to_string())],
                Some("input_json_delta") => vec![Event::default().data(json!({
                    "id": request_id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model,
                    "choices": [{
                        "index": 0,
                        "delta": {"tool_calls": [{
                            "index": value.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                            "function": {"arguments": delta.get("partial_json").cloned().unwrap_or(json!(""))}
                        }]},
                        "finish_reason": null
                    }]
                }).to_string())],
                _ => Vec::new(),
            }
        }
        "content_block_start" => {
            let block = value
                .get("content_block")
                .unwrap_or(&serde_json::Value::Null);
            if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                vec![Event::default().data(json!({
                    "id": request_id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model,
                    "choices": [{
                        "index": 0,
                        "delta": {"tool_calls": [{
                            "index": value.get("index").and_then(|v| v.as_i64()).unwrap_or(0),
                            "id": block.get("id").cloned().unwrap_or(json!(Uuid::new_v4().to_string())),
                            "type": "function",
                            "function": {"name": block.get("name").cloned().unwrap_or(json!("tool")), "arguments": ""}
                        }]},
                        "finish_reason": null
                    }]
                }).to_string())]
            } else {
                Vec::new()
            }
        }
        "message_delta" => {
            let finish_reason = match value
                .get("delta")
                .and_then(|v| v.get("stop_reason"))
                .and_then(|v| v.as_str())
            {
                Some("tool_use") => "tool_calls",
                Some("max_tokens") => "length",
                _ => "stop",
            };
            vec![Event::default().data(
                json!({
                    "id": request_id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model,
                    "choices": [{"index": 0, "delta": {}, "finish_reason": finish_reason}]
                })
                .to_string(),
            )]
        }
        "message_stop" => vec![Event::default().data("[DONE]")],
        _ => Vec::new(),
    }
}

fn openai_chat_chunk_to_anthropic_events(value: &serde_json::Value) -> Vec<Event> {
    let choice = value
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|choices| choices.first())
        .unwrap_or(&serde_json::Value::Null);
    let delta = choice.get("delta").unwrap_or(&serde_json::Value::Null);
    let mut events = Vec::new();

    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
        if !content.is_empty() {
            events.push(
                Event::default().event("content_block_delta").data(
                    json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": {"type": "text_delta", "text": content}
                    })
                    .to_string(),
                ),
            );
        }
    }

    if let Some(tool_calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
        for call in tool_calls {
            let index = call.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
            if call.get("id").is_some()
                || call.get("function").and_then(|f| f.get("name")).is_some()
            {
                events.push(Event::default().event("content_block_start").data(json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": {
                        "type": "tool_use",
                        "id": call.get("id").cloned().unwrap_or(json!(Uuid::new_v4().to_string())),
                        "name": call.get("function").and_then(|f| f.get("name")).cloned().unwrap_or(json!("tool")),
                        "input": {}
                    }
                }).to_string()));
            }
            if let Some(arguments) = call
                .get("function")
                .and_then(|function| function.get("arguments"))
                .and_then(|v| v.as_str())
            {
                if !arguments.is_empty() {
                    events.push(
                        Event::default().event("content_block_delta").data(
                            json!({
                                "type": "content_block_delta",
                                "index": index,
                                "delta": {"type": "input_json_delta", "partial_json": arguments}
                            })
                            .to_string(),
                        ),
                    );
                }
            }
        }
    }

    if choice
        .get("finish_reason")
        .is_some_and(|value| !value.is_null())
    {
        events.push(
            Event::default().event("content_block_stop").data(
                json!({
                    "type": "content_block_stop",
                    "index": 0
                })
                .to_string(),
            ),
        );
        events.push(
            Event::default().event("message_delta").data(
                json!({
                    "type": "message_delta",
                    "delta": {"stop_reason": openai_finish_reason_to_anthropic(choice)},
                    "usage": {"output_tokens": 0}
                })
                .to_string(),
            ),
        );
        events.push(
            Event::default()
                .event("message_stop")
                .data(json!({"type": "message_stop"}).to_string()),
        );
    }

    events
}

fn openai_finish_reason_to_anthropic(choice: &serde_json::Value) -> &'static str {
    match choice.get("finish_reason").and_then(|v| v.as_str()) {
        Some("tool_calls") | Some("function_call") => "tool_use",
        Some("length") => "max_tokens",
        _ => "end_turn",
    }
}

fn stream_payload_has_visible_delta(value: &serde_json::Value, transform: StreamTransform) -> bool {
    match transform {
        StreamTransform::OpenAIChatToOpenAIChat
        | StreamTransform::OpenAIChatToAnthropic
        | StreamTransform::OpenAIResponses => openai_stream_payload_has_visible_delta(value),
        StreamTransform::AnthropicToAnthropic | StreamTransform::AnthropicToOpenAIChat => {
            anthropic_stream_payload_has_visible_delta(value)
        }
    }
}

fn openai_stream_payload_has_visible_delta(value: &serde_json::Value) -> bool {
    if let Some(output) = value.get("output_text").and_then(|v| v.as_str()) {
        return !output.is_empty();
    }

    value
        .get("choices")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .any(|choice| {
            let delta = choice.get("delta").unwrap_or(&serde_json::Value::Null);
            delta
                .get("content")
                .and_then(|v| v.as_str())
                .is_some_and(|text| !text.is_empty())
                || delta
                    .get("tool_calls")
                    .and_then(|v| v.as_array())
                    .is_some_and(|calls| !calls.is_empty())
        })
}

fn anthropic_stream_payload_has_visible_delta(value: &serde_json::Value) -> bool {
    match value.get("type").and_then(|v| v.as_str()) {
        Some("content_block_start") => {
            value
                .get("content_block")
                .and_then(|block| block.get("type"))
                .and_then(|v| v.as_str())
                == Some("tool_use")
        }
        Some("content_block_delta") => value
            .get("delta")
            .and_then(|delta| delta.get("type"))
            .and_then(|v| v.as_str())
            .is_some_and(|delta_type| matches!(delta_type, "text_delta" | "input_json_delta")),
        _ => false,
    }
}

fn stream_usage_tokens(value: &serde_json::Value) -> (i64, i64) {
    let usage = value
        .get("usage")
        .or_else(|| {
            value
                .get("message")
                .and_then(|message| message.get("usage"))
        })
        .unwrap_or(&serde_json::Value::Null);
    let input = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .or_else(|| {
            usage
                .get("input_tokens_details")
                .and_then(|v| v.get("cached_tokens"))
        })
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    (input, output)
}

fn record_stream_compat_request(
    state: &CompatApiState,
    request_id: &str,
    provider: &str,
    model: &str,
    input_tokens: i64,
    output_tokens: i64,
    ttft_ms: Option<i64>,
    duration_ms: i64,
    status: &str,
    request_summary_json: Option<String>,
    error_text: Option<String>,
    started_at: String,
) {
    let request_type = RequestType::Stream;
    let record = RequestRecordUpsertRecord {
        id: request_id.to_string(),
        provider: provider.to_string(),
        source_mode: "local_compat_api".to_string(),
        session_id: None,
        request_id: Some(request_id.to_string()),
        model: Some(model.to_string()),
        is_stream: request_type.is_stream(),
        input_tokens,
        output_tokens,
        cached_input_tokens: 0,
        reasoning_tokens: 0,
        ttft_ms,
        duration_ms: Some(duration_ms),
        status: status.to_string(),
        started_at,
        finished_at: Some(Utc::now().to_rfc3339()),
        request_summary_json,
        response_summary_json: None,
        error_text,
    };

    if let Ok(conn) = db::get_connection(&state.app_handle) {
        let _ = db::upsert_request_record(&conn, &record);
        if status == "success" {
            let _ = db::rebuild_daily_usage_for_provider(&conn, provider);
        }
    }
}

fn resolve_upstream_protocol(
    state: &CompatApiState,
    model: &str,
    request_protocol: RequestProtocol,
) -> Option<UpstreamProtocol> {
    let preferred = match request_protocol {
        RequestProtocol::OpenAI => UpstreamProtocol::OpenAI,
        RequestProtocol::Anthropic => UpstreamProtocol::Anthropic,
    };
    let fallback = match request_protocol {
        RequestProtocol::OpenAI => UpstreamProtocol::Anthropic,
        RequestProtocol::Anthropic => UpstreamProtocol::OpenAI,
    };

    find_matching_profile(state, model, preferred)
        .map(|_| preferred)
        .or_else(|| find_matching_profile(state, model, fallback).map(|_| fallback))
}

fn record_compat_request<TReq: serde::Serialize, TResp: serde::Serialize>(
    state: &CompatApiState,
    request_id: &str,
    provider: &str,
    model: &str,
    is_stream: bool,
    input_tokens: i64,
    output_tokens: i64,
    duration_ms: i64,
    status: &str,
    request: Option<&TReq>,
    response: Option<&TResp>,
    error_text: Option<String>,
) {
    let request_type = if is_stream {
        RequestType::Stream
    } else {
        RequestType::Sync
    };
    let record = RequestRecordUpsertRecord {
        id: request_id.to_string(),
        provider: provider.to_string(),
        source_mode: "local_compat_api".to_string(),
        session_id: None,
        request_id: Some(request_id.to_string()),
        model: Some(model.to_string()),
        is_stream: request_type.is_stream(),
        input_tokens,
        output_tokens,
        cached_input_tokens: 0,
        reasoning_tokens: 0,
        ttft_ms: None,
        duration_ms: Some(duration_ms),
        status: status.to_string(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: Some(Utc::now().to_rfc3339()),
        request_summary_json: request.and_then(|value| serde_json::to_string(value).ok()),
        response_summary_json: response.and_then(|value| serde_json::to_string(value).ok()),
        error_text,
    };

    if let Ok(conn) = db::get_connection(&state.app_handle) {
        let _ = db::upsert_request_record(&conn, &record);
        if status == "success" {
            let _ = db::rebuild_daily_usage_for_provider(&conn, provider);
        }
    }
}

fn openai_error_response(
    request_id: String,
    model: String,
    message: &str,
) -> Json<OpenAIChatCompletionResponse> {
    Json(OpenAIChatCompletionResponse {
        id: request_id,
        object: "chat.completion".to_string(),
        created: Utc::now().timestamp(),
        model,
        choices: vec![OpenAIChatChoice {
            index: 0,
            message: OpenAIChatMessage {
                role: "assistant".to_string(),
                content: json!(message),
                extra: serde_json::Map::new(),
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

fn anthropic_tools_to_openai(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.get("name").cloned().unwrap_or(json!("tool")),
                    "description": tool.get("description").cloned().unwrap_or(json!("")),
                    "parameters": tool.get("input_schema").cloned().unwrap_or(json!({"type": "object"})),
                }
            })
        })
        .collect()
}

fn openai_tools_to_anthropic(tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .filter_map(|tool| {
            let function = tool.get("function").unwrap_or(tool);
            let name = function.get("name")?.clone();
            Some(json!({
                "name": name,
                "description": function.get("description").cloned().unwrap_or(json!("")),
                "input_schema": function.get("parameters").cloned().unwrap_or(json!({"type": "object"})),
            }))
        })
        .collect()
}

fn anthropic_messages_to_openai(
    system: &Option<serde_json::Value>,
    messages: &[AnthropicMessage],
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if let Some(system) = system {
        out.push(json!({"role": "system", "content": content_to_text(system)}));
    }

    for message in messages {
        let blocks = message.content.as_array();
        if message.role == "assistant" {
            let text = anthropic_text_from_content(&message.content);
            let tool_calls: Vec<_> = blocks
                .into_iter()
                .flatten()
                .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                .map(|block| {
                    json!({
                        "id": block.get("id").cloned().unwrap_or(json!(Uuid::new_v4().to_string())),
                        "type": "function",
                        "function": {
                            "name": block.get("name").cloned().unwrap_or(json!("tool")),
                            "arguments": block.get("input").map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string()),
                        }
                    })
                })
                .collect();
            let mut obj = json!({"role": "assistant", "content": text});
            if !tool_calls.is_empty() {
                obj["tool_calls"] = json!(tool_calls);
            }
            out.push(obj);
        } else {
            let mut pushed_tool_result = false;
            if let Some(blocks) = blocks {
                for block in blocks {
                    if block.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                        out.push(json!({
                            "role": "tool",
                            "tool_call_id": block.get("tool_use_id").cloned().unwrap_or(json!("")),
                            "content": anthropic_text_from_content(block.get("content").unwrap_or(&json!(""))),
                        }));
                        pushed_tool_result = true;
                    }
                }
            }
            let text = anthropic_text_from_content(&message.content);
            if !text.is_empty() || !pushed_tool_result {
                out.push(json!({"role": message.role, "content": text}));
            }
        }
    }

    out
}

fn openai_messages_to_anthropic(
    messages: &[OpenAIChatMessage],
) -> (Option<serde_json::Value>, Vec<serde_json::Value>) {
    let mut system_parts = Vec::new();
    let mut out = Vec::new();

    for message in messages {
        match message.role.as_str() {
            "system" => system_parts.push(content_to_text(&message.content)),
            "tool" => out.push(json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": message.extra.get("tool_call_id").cloned().unwrap_or(json!("")),
                    "content": content_to_text(&message.content),
                }],
            })),
            "assistant" => {
                let mut content = Vec::new();
                let text = content_to_text(&message.content);
                if !text.is_empty() {
                    content.push(json!({"type": "text", "text": text}));
                }
                if let Some(tool_calls) = message.extra.get("tool_calls").and_then(|v| v.as_array())
                {
                    for call in tool_calls {
                        let function = call.get("function").unwrap_or(call);
                        let input = function
                            .get("arguments")
                            .and_then(|v| v.as_str())
                            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                            .unwrap_or(json!({}));
                        content.push(json!({
                            "type": "tool_use",
                            "id": call.get("id").cloned().unwrap_or(json!(Uuid::new_v4().to_string())),
                            "name": function.get("name").cloned().unwrap_or(json!("tool")),
                            "input": input,
                        }));
                    }
                }
                out.push(json!({"role": "assistant", "content": content}));
            }
            _ => out.push(json!({"role": "user", "content": content_to_text(&message.content)})),
        }
    }

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(json!(system_parts.join("\n\n")))
    };
    (system, out)
}

fn openai_chat_body_to_anthropic_content(body: &serde_json::Value) -> Vec<serde_json::Value> {
    let message = body
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .unwrap_or(&json!({}))
        .clone();
    let mut content = Vec::new();
    let text = message
        .get("content")
        .map(content_to_text)
        .unwrap_or_default();
    if !text.is_empty() {
        content.push(json!({"type": "text", "text": text}));
    }
    if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
        for call in tool_calls {
            let function = call.get("function").unwrap_or(call);
            let input = function
                .get("arguments")
                .and_then(|v| v.as_str())
                .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                .unwrap_or(json!({}));
            content.push(json!({
                "type": "tool_use",
                "id": call.get("id").cloned().unwrap_or(json!(Uuid::new_v4().to_string())),
                "name": function.get("name").cloned().unwrap_or(json!("tool")),
                "input": input,
            }));
        }
    }
    if content.is_empty() {
        content.push(json!({"type": "text", "text": ""}));
    }
    content
}

fn anthropic_body_to_openai_message(body: &serde_json::Value) -> OpenAIChatMessage {
    let content = body.get("content").cloned().unwrap_or(json!([]));
    let text = anthropic_text_from_content(&content);
    let tool_calls: Vec<_> = content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
        .map(|block| {
            json!({
                "id": block.get("id").cloned().unwrap_or(json!(Uuid::new_v4().to_string())),
                "type": "function",
                "function": {
                    "name": block.get("name").cloned().unwrap_or(json!("tool")),
                    "arguments": block.get("input").map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string()),
                }
            })
        })
        .collect();
    let mut extra = serde_json::Map::new();
    if !tool_calls.is_empty() {
        extra.insert("tool_calls".to_string(), json!(tool_calls));
    }
    OpenAIChatMessage {
        role: "assistant".to_string(),
        content: json!(text),
        extra,
    }
}

fn anthropic_stop_reason_to_openai(body: &serde_json::Value) -> String {
    match body.get("stop_reason").and_then(|v| v.as_str()) {
        Some("tool_use") => "tool_calls",
        Some("max_tokens") => "length",
        _ => "stop",
    }
    .to_string()
}

fn anthropic_text_from_content(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| {
                if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                    block
                        .get("text")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                } else if block.is_string() {
                    block.as_str().map(str::to_string)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn content_to_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| value.as_str().unwrap_or("").to_string())
            })
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
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
            return Some((1, 0));
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
