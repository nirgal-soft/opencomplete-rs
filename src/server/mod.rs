//! HTTP Server for Neovim plugin communication
//!
//! Exposes a simple JSON API that the Neovim Lua plugin can call:
//! - POST /complete - Stream a completion (SSE)
//! - GET /complete/ws - Stream a completion (WebSocket)
//! - GET /health - Check server status and list providers

use crate::providers::{CompletionRequest, CompletionStream, ProviderRegistry};
use axum::{
    Json, Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

// ─────────────────────────────────────────────────────────────────────────────
// Server State
// ─────────────────────────────────────────────────────────────────────────────

pub struct AppState {
    pub providers: RwLock<ProviderRegistry>,
}

// ─────────────────────────────────────────────────────────────────────────────
// API Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CompleteRequest {
    /// Which provider to use (defaults to default provider)
    #[serde(default)]
    provider: Option<String>,

    /// The completion request
    #[serde(flatten)]
    request: CompletionRequest,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    providers: Vec<ProviderInfo>,
}

#[derive(Debug, Serialize)]
struct ProviderInfo {
    id: String,
    name: String,
    is_default: bool,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Health check endpoint
async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let providers = state.providers.read().await;

    let provider_list: Vec<ProviderInfo> = providers
        .list()
        .iter()
        .map(|p| ProviderInfo {
            id: p.id().to_string(),
            name: p.display_name().to_string(),
            is_default: providers
                .get_default()
                .map(|d| d.id() == p.id())
                .unwrap_or(false),
        })
        .collect();

    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        providers: provider_list,
    })
}

/// SSE-based completion endpoint
async fn complete_sse(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CompleteRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    let providers = state.providers.read().await;

    let provider = match &req.provider {
        Some(id) => providers.get(id),
        None => providers.get_default(),
    }
    .ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No provider available".to_string(),
                code: "no_provider".to_string(),
            }),
        )
    })?;

    drop(providers);

    let stream = provider.complete(req.request).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "completion_error".to_string(),
            }),
        )
    })?;

    let sse_stream = stream.map(|result| {
        Ok(match result {
            Ok(chunk) => Event::default().json_data(&chunk).unwrap(),
            Err(e) => Event::default()
                .json_data(&ErrorResponse {
                    error: e.to_string(),
                    code: "stream_error".to_string(),
                })
                .unwrap(),
        })
    });

    Ok(Sse::new(sse_stream))
}

/// WebSocket-based completion endpoint (alternative for better latency)
async fn complete_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    while let Some(msg) = socket.recv().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            _ => continue,
        };

        let req: CompleteRequest = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                let error = serde_json::to_string(&ErrorResponse {
                    error: e.to_string(),
                    code: "parse_error".to_string(),
                })
                .unwrap();
                let _ = socket.send(Message::Text(error.into())).await;
                continue;
            }
        };

        let providers = state.providers.read().await;
        let provider = match &req.provider {
            Some(id) => providers.get(id),
            None => providers.get_default(),
        };

        let provider = match provider {
            Some(p) => p,
            None => {
                let error = serde_json::to_string(&ErrorResponse {
                    error: "No provider available".to_string(),
                    code: "no_provider".to_string(),
                })
                .unwrap();
                let _ = socket.send(Message::Text(error.into())).await;
                continue;
            }
        };

        drop(providers);

        let stream: CompletionStream = match provider.complete(req.request).await {
            Ok(s) => s,
            Err(e) => {
                let error = serde_json::to_string(&ErrorResponse {
                    error: e.to_string(),
                    code: "completion_error".to_string(),
                })
                .unwrap();
                let _ = socket.send(Message::Text(error.into())).await;
                continue;
            }
        };

        futures::pin_mut!(stream);

        while let Some(result) = stream.next().await {
            let json = match result {
                Ok(chunk) => serde_json::to_string(&chunk).unwrap(),
                Err(e) => serde_json::to_string(&ErrorResponse {
                    error: e.to_string(),
                    code: "stream_error".to_string(),
                })
                .unwrap(),
            };

            if socket.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Server Builder
// ─────────────────────────────────────────────────────────────────────────────

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/complete", post(complete_sse))
        .route("/complete/ws", get(complete_ws))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn run_server(state: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    tracing::info!("Server listening on http://127.0.0.1:{}", port);

    axum::serve(listener, app).await?;

    Ok(())
}
