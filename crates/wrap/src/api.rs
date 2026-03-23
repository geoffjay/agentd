//! REST API handlers for the wrap service.
//!
//! This module provides HTTP endpoints for launching and managing agent sessions.
//! It uses the Axum web framework and the [`ExecutionBackend`] abstraction so the
//! same API works with tmux, Docker, and PTY backends.
//!
//! # API Endpoints
//!
//! - `GET /health`                       — Health check
//! - `GET /info`                         — Active backend type and capabilities
//! - `POST /launch`                      — Launch an agent session
//! - `GET /sessions`                     — List active sessions
//! - `GET /sessions/{name}`              — Get a specific session
//! - `DELETE /sessions/{name}`           — Kill a session
//! - `GET /sessions/{name}/terminal`     — WebSocket PTY terminal relay (PTY only)

use crate::{
    backend::{ExecutionBackend, SessionConfig},
    types::*,
};
use axum::{
    extract::{ws::WebSocketUpgrade, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use std::sync::Arc;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Shared application state injected into all API handlers.
#[derive(Clone)]
pub struct AppState {
    /// The active execution backend (tmux / docker / pty).
    pub backend: Arc<dyn ExecutionBackend>,
    /// The type of the active backend — used for capability reporting.
    pub backend_type: BackendType,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Creates and configures the Axum router with all API endpoints.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/info", axum::routing::get(backend_info))
        .route("/launch", axum::routing::post(launch_session))
        .route("/sessions", axum::routing::get(list_sessions))
        .route("/sessions/{name}", axum::routing::get(get_session).delete(kill_session))
        .route("/sessions/{name}/terminal", axum::routing::get(terminal_ws))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /health` — service liveness.
async fn health_check() -> impl IntoResponse {
    Json(HealthResponse::ok("agentd-wrap", env!("CARGO_PKG_VERSION")))
}

/// `GET /info` — active backend type and capabilities.
async fn backend_info(State(state): State<AppState>) -> impl IntoResponse {
    let caps = state.backend_type.capabilities();
    Json(BackendInfo {
        backend_type: state.backend_type,
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: caps,
    })
}

/// `POST /launch` — create and start an agent session.
async fn launch_session(
    State(state): State<AppState>,
    Json(req): Json<LaunchRequest>,
) -> Result<Json<LaunchResponse>, ApiError> {
    info!(
        "Launching agent session: session={}, agent={}, model={}/{}, backend={}",
        req.project_name, req.agent_type, req.model_provider, req.model_name, state.backend_type,
    );

    // Validate project path exists
    if !std::path::Path::new(&req.project_path).exists() {
        error!("Project path does not exist: {}", req.project_path);
        return Ok(Json(LaunchResponse {
            success: false,
            session_name: Some(req.project_name.clone()),
            message: format!("Project path does not exist: {}", req.project_path),
            error: Some(format!("Project path does not exist: {}", req.project_path)),
        }));
    }

    // Derive session name (timestamp suffix if project name is empty)
    let session_name = if req.project_name.is_empty() {
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        format!("{}-{}", state.backend.prefix(), ts)
    } else {
        req.project_name.clone()
    };

    let config = SessionConfig {
        session_name: session_name.clone(),
        working_dir: req.project_path.clone(),
        agent_type: req.agent_type.clone(),
        model_provider: req.model_provider.clone(),
        model_name: req.model_name.clone(),
        layout: req.layout.clone(),
        network_policy: None,
    };

    // Create session, then launch agent inside it
    if let Err(e) = state.backend.create_session(&config).await {
        error!("Failed to create session '{}': {}", session_name, e);
        return Ok(Json(LaunchResponse {
            success: false,
            session_name: Some(session_name.clone()),
            message: format!("Failed to create session: {e}"),
            error: Some(e.to_string()),
        }));
    }
    info!("Created session: {}", session_name);

    if let Err(e) = state.backend.launch_agent(&config).await {
        error!("Failed to launch agent in '{}': {}", session_name, e);
        let _ = state.backend.kill_session(&session_name).await;
        return Ok(Json(LaunchResponse {
            success: false,
            session_name: Some(session_name.clone()),
            message: format!("Failed to launch agent: {e}"),
            error: Some(e.to_string()),
        }));
    }
    info!("Successfully launched agent in session: {}", session_name);

    Ok(Json(LaunchResponse {
        success: true,
        session_name: Some(session_name.clone()),
        message: format!("Agent launched successfully in session: {session_name}"),
        error: None,
    }))
}

/// `GET /sessions` — list all active sessions with backend metadata.
async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<SessionListResponse>, ApiError> {
    let names = state.backend.list_sessions().await.map_err(|e| {
        error!("Failed to list sessions: {}", e);
        ApiError::Internal(e)
    })?;

    let caps = state.backend_type.capabilities();
    let sessions: Vec<SessionInfo> = names
        .into_iter()
        .map(|name| SessionInfo {
            name,
            active: true,
            backend: state.backend_type.clone(),
            capabilities: caps.clone(),
        })
        .collect();

    let count = sessions.len();
    Ok(Json(SessionListResponse { sessions, count }))
}

/// `GET /sessions/{name}` — get info for a single session.
async fn get_session(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<SessionInfo>, ApiError> {
    let exists = state.backend.session_exists(&name).await.map_err(|e| {
        error!("Failed to check session '{}': {}", name, e);
        ApiError::Internal(e)
    })?;

    if exists {
        let caps = state.backend_type.capabilities();
        Ok(Json(SessionInfo {
            name,
            active: true,
            backend: state.backend_type.clone(),
            capabilities: caps,
        }))
    } else {
        Err(ApiError::NotFound(format!("Session '{}' not found", name)))
    }
}

/// `DELETE /sessions/{name}` — kill a session.
async fn kill_session(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<KillSessionResponse>, ApiError> {
    let exists = state.backend.session_exists(&name).await.map_err(|e| {
        error!("Failed to check session '{}': {}", name, e);
        ApiError::Internal(e)
    })?;

    if !exists {
        return Err(ApiError::NotFound(format!("Session '{}' not found", name)));
    }

    state.backend.kill_session(&name).await.map_err(|e| {
        error!("Failed to kill session '{}': {}", name, e);
        ApiError::Internal(e)
    })?;

    info!("Killed session: {}", name);
    Ok(Json(KillSessionResponse {
        success: true,
        message: format!("Session '{}' terminated", name),
    }))
}

// ---------------------------------------------------------------------------
// PTY WebSocket terminal relay
// ---------------------------------------------------------------------------

/// `GET /sessions/{name}/terminal` — WebSocket PTY terminal relay.
///
/// Streams raw PTY output to the WebSocket client as binary frames and
/// forwards binary frames from the client as PTY stdin. Text frames are
/// interpreted as JSON control messages (currently `{"type":"resize","cols":N,"rows":N}`).
///
/// Returns `400 Bad Request` for non-PTY backends, or `404` if the session
/// does not exist or has no PTY stream.
async fn terminal_ws(
    ws: WebSocketUpgrade,
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Only PTY backend supports terminal streaming
    if state.backend_type != BackendType::Pty {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!(
                    "Terminal attach requires PTY backend; active backend is '{}'",
                    state.backend_type
                )
            })),
        )
            .into_response();
    }

    // Look up the PTY output stream
    match state.backend.session_output_stream(&name).await {
        Err(e) => {
            error!("Failed to get PTY stream for session '{}': {}", name, e);
            ApiError::Internal(e).into_response()
        }
        Ok(None) => {
            ApiError::NotFound(format!("Session '{}' not found or has no PTY stream", name))
                .into_response()
        }
        Ok(Some(stream)) => {
            let backend = Arc::clone(&state.backend);
            let session_name = name.clone();
            ws.on_upgrade(move |socket| handle_terminal_ws(socket, stream, backend, session_name))
        }
    }
}

/// Drive the bidirectional PTY ↔ WebSocket relay.
async fn handle_terminal_ws(
    mut socket: axum::extract::ws::WebSocket,
    stream: crate::pty_stream::PtyOutputStream,
    backend: Arc<dyn ExecutionBackend>,
    session_name: String,
) {
    use axum::extract::ws::Message;

    let (history, mut rx) = stream.subscribe();

    // Replay buffered history so the client sees prior output immediately
    for chunk in history {
        if socket.send(Message::Binary(chunk)).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            // PTY output → WebSocket (binary frames)
            result = rx.recv() => {
                match result {
                    Ok(chunk) => {
                        if socket.send(Message::Binary(chunk)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WS terminal client lagged {} messages for session '{}'", n, session_name);
                        // Continue — the client just missed some output
                    }
                }
            }
            // WebSocket → PTY stdin
            result = socket.recv() => {
                match result {
                    Some(Ok(Message::Binary(data))) => {
                        if let Err(e) = stream.write_input(&data) {
                            warn!("Failed to write to PTY '{}': {}", session_name, e);
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        // JSON control frame: {"type":"resize","cols":N,"rows":N}
                        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&text) {
                            if msg.get("type").and_then(|t| t.as_str()) == Some("resize") {
                                if let (Some(cols), Some(rows)) = (
                                    msg.get("cols").and_then(|v| v.as_u64()),
                                    msg.get("rows").and_then(|v| v.as_u64()),
                                ) {
                                    let _ = backend
                                        .resize_session(&session_name, cols as u16, rows as u16)
                                        .await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    Some(Ok(_)) => {} // Ping/Pong handled automatically by axum
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// API error types for the wrap service.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Internal server error (HTTP 500)
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    /// Resource not found (HTTP 404)
    #[error("not found: {0}")]
    NotFound(String),
    /// Invalid input or request error (HTTP 400)
    #[error("invalid input: {0}")]
    #[allow(dead_code)]
    InvalidInput(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::InvalidInput(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
