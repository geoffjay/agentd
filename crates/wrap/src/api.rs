//! REST API handlers for the wrap service.
//!
//! This module provides HTTP endpoints for launching and managing agent sessions
//! in tmux environments. It uses the Axum web framework to expose a REST API
//! for the agentd-wrap service.
//!
//! # API Endpoints
//!
//! - `GET /health` - Health check endpoint
//! - `POST /launch` - Launch an agent session in tmux
//!
//! # Examples
//!
//! ## Creating a Router
//!
//! ```no_run
//! use wrap::api::create_router;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let router = create_router();
//!
//!     // Bind and serve
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:17005").await?;
//!     axum::serve(listener, router).await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Making Requests
//!
//! ```bash
//! # Health check
//! curl http://localhost:17005/health
//!
//! # Launch an agent session
//! curl -X POST http://localhost:17005/launch \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "session_name": "my-project",
//!     "path": "/home/user/projects/my-project",
//!     "agent": "claude-code",
//!     "provider": "anthropic",
//!     "model": "claude-sonnet-4.5"
//!   }'
//! ```

use crate::{tmux::TmuxManager, types::*};
use axum::{extract::Path, http::StatusCode, response::IntoResponse, Json, Router};
use tracing::{error, info};

/// Creates and configures the Axum router with all API endpoints.
///
/// Sets up all HTTP routes for the wrap service. The router is
/// ready to be served by Axum's `serve` function.
///
/// # Returns
///
/// Returns a configured [`Router`] ready to serve HTTP requests.
///
/// # Examples
///
/// ```no_run
/// use wrap::api::create_router;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let router = create_router();
///
///     let listener = tokio::net::TcpListener::bind("127.0.0.1:17005").await?;
///     axum::serve(listener, router).await?;
///     Ok(())
/// }
/// ```
pub fn create_router() -> Router {
    Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/launch", axum::routing::post(launch_session))
        .route("/sessions", axum::routing::get(list_sessions))
        .route("/sessions/{name}", axum::routing::get(get_session).delete(kill_session))
}

/// Health check endpoint handler.
///
/// Returns basic service information including status and version.
/// This endpoint is useful for monitoring and load balancer health checks.
///
/// # Endpoint
///
/// `GET /health`
///
/// # Response
///
/// Returns HTTP 200 with JSON body:
/// ```json
/// {
///   "status": "ok",
///   "version": "0.1.0"
/// }
/// ```
///
/// # Examples
///
/// ```bash
/// curl http://localhost:17005/health
/// ```
async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    })
}

/// Launch an agent session in a tmux environment.
///
/// Creates a new tmux session for the specified project and agent configuration.
/// The session is created in detached mode and can be attached to by the user.
///
/// # Endpoint
///
/// `POST /launch`
///
/// # Request Body
///
/// JSON object with the following fields:
/// - `session_name` - Unique name for the tmux session
/// - `path` - Absolute path to the project directory
/// - `agent` - Type of agent to launch (claude-code, crush, opencode, gemini, general)
/// - `provider` - Model provider (anthropic, openai, ollama, etc.)
/// - `model` - Name of the model to use
/// - `layout` (optional) - Tmux layout configuration as JSON string
/// - `env` (optional) - Additional environment variables
///
/// # Response
///
/// Returns HTTP 200 with JSON body containing:
/// - `session_id` - Unique identifier for the session
/// - `session_name` - Name of the created tmux session
/// - `success` - Whether the launch was successful
/// - `error` (optional) - Error message if launch failed
/// - `pid` (optional) - Process ID if available
///
/// # Errors
///
/// - HTTP 400 - Invalid request body
/// - HTTP 500 - Failed to create tmux session or launch agent
///
/// # Examples
///
/// ```bash
/// curl -X POST http://localhost:17005/launch \
///   -H "Content-Type: application/json" \
///   -d '{
///     "session_name": "my-project",
///     "path": "/home/user/projects/my-project",
///     "agent": "claude-code",
///     "provider": "anthropic",
///     "model": "claude-sonnet-4.5"
///   }'
/// ```
async fn launch_session(Json(req): Json<LaunchRequest>) -> Result<Json<LaunchResponse>, ApiError> {
    info!(
        "Launching agent session: session={}, agent={}, model={}/{}",
        req.project_name, req.agent_type, req.model_provider, req.model_name
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

    // Create tmux manager
    let tmux = TmuxManager::new("agentd");

    // Use the provided session name or generate one
    let session_name = if req.project_name.is_empty() {
        let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        format!("{}-{}", tmux.prefix(), timestamp)
    } else {
        req.project_name.clone()
    };

    // Create tmux session with layout if provided
    match tmux.create_session(&session_name, &req.project_path, req.layout.as_ref()) {
        Ok(_) => {
            info!("Created tmux session: {}", session_name);

            // Launch agent in the session
            match launch_agent(&tmux, &session_name, &req) {
                Ok(_) => {
                    info!("Successfully launched agent in session: {}", session_name);
                    Ok(Json(LaunchResponse {
                        success: true,
                        session_name: Some(session_name.clone()),
                        message: format!(
                            "Agent launched successfully in session: {}",
                            session_name
                        ),
                        error: None,
                    }))
                }
                Err(e) => {
                    error!("Failed to launch agent: {}", e);
                    // Kill the session since agent launch failed
                    let _ = tmux.kill_session(&session_name);
                    Ok(Json(LaunchResponse {
                        success: false,
                        session_name: Some(session_name.clone()),
                        message: format!("Failed to launch agent: {}", e),
                        error: Some(e.to_string()),
                    }))
                }
            }
        }
        Err(e) => {
            error!("Failed to create tmux session: {}", e);
            Ok(Json(LaunchResponse {
                success: false,
                session_name: Some(session_name.clone()),
                message: format!("Failed to create tmux session: {}", e),
                error: Some(e.to_string()),
            }))
        }
    }
}

/// Launch the specified agent in a tmux session.
///
/// This function sends the appropriate commands to start the agent CLI
/// based on the agent type and configuration provided in the request.
///
/// # Arguments
///
/// * `tmux` - The tmux manager instance
/// * `session_name` - Name of the tmux session to launch the agent in
/// * `req` - The launch request containing agent configuration
///
/// # Returns
///
/// Returns `Ok(())` if the agent was launched successfully, or an error
/// if the agent type is unsupported or the launch command fails.
fn launch_agent(tmux: &TmuxManager, session_name: &str, req: &LaunchRequest) -> anyhow::Result<()> {
    // Build the agent launch command based on agent type
    let command = match req.agent_type.as_str() {
        "claude-code" => {
            // Launch Claude Code CLI
            "claude".to_string()
        }
        "crush" => {
            // Launch Crush CLI
            "crush".to_string()
        }
        "opencode" => {
            // Launch opencode CLI
            format!("opencode --model-provider {} --model {}", req.model_provider, req.model_name)
        }
        "gemini" => {
            // Launch Gemini CLI
            format!("gemini --model {}", req.model_name)
        }
        "general" => {
            // General purpose agent - just start a shell
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported agent type: {}", req.agent_type));
        }
    };

    // Send the command to the tmux session
    tmux.send_command(session_name, &command)?;

    Ok(())
}

/// List all active tmux sessions.
///
/// # Endpoint
///
/// `GET /sessions`
///
/// # Response
///
/// Returns HTTP 200 with a list of active sessions.
async fn list_sessions() -> Result<Json<SessionListResponse>, ApiError> {
    let tmux = TmuxManager::new("agentd");

    let session_names = tmux.list_sessions().map_err(|e| {
        error!("Failed to list sessions: {}", e);
        ApiError::Internal(e)
    })?;

    let sessions: Vec<SessionInfo> =
        session_names.into_iter().map(|name| SessionInfo { name, active: true }).collect();

    let count = sessions.len();

    Ok(Json(SessionListResponse { sessions, count }))
}

/// Get the status of a specific tmux session.
///
/// # Endpoint
///
/// `GET /sessions/{name}`
///
/// # Response
///
/// Returns HTTP 200 with session info if found, or HTTP 404 if not found.
async fn get_session(Path(name): Path<String>) -> Result<Json<SessionInfo>, ApiError> {
    let tmux = TmuxManager::new("agentd");

    let exists = tmux.session_exists(&name).map_err(|e| {
        error!("Failed to check session: {}", e);
        ApiError::Internal(e)
    })?;

    if exists {
        Ok(Json(SessionInfo { name, active: true }))
    } else {
        Err(ApiError::NotFound(format!("Session '{}' not found", name)))
    }
}

/// Kill/terminate a specific tmux session.
///
/// # Endpoint
///
/// `DELETE /sessions/{name}`
///
/// # Response
///
/// Returns HTTP 200 with success status, or HTTP 404 if session not found.
async fn kill_session(Path(name): Path<String>) -> Result<Json<KillSessionResponse>, ApiError> {
    let tmux = TmuxManager::new("agentd");

    // Check if session exists first
    let exists = tmux.session_exists(&name).map_err(|e| {
        error!("Failed to check session: {}", e);
        ApiError::Internal(e)
    })?;

    if !exists {
        return Err(ApiError::NotFound(format!("Session '{}' not found", name)));
    }

    tmux.kill_session(&name).map_err(|e| {
        error!("Failed to kill session: {}", e);
        ApiError::Internal(e)
    })?;

    info!("Killed tmux session: {}", name);

    Ok(Json(KillSessionResponse {
        success: true,
        message: format!("Session '{}' terminated", name),
    }))
}

// === Error Handling ===

/// API error types that can be returned from handlers.
///
/// These errors are automatically converted to appropriate HTTP responses
/// with status codes and JSON error messages.
#[derive(Debug)]
pub enum ApiError {
    /// Internal server error (HTTP 500)
    Internal(anyhow::Error),
    /// Resource not found (HTTP 404)
    NotFound(String),
    /// Invalid input or request error (HTTP 400)
    #[allow(dead_code)]
    InvalidInput(String),
}

impl IntoResponse for ApiError {
    /// Converts the error into an HTTP response.
    ///
    /// Maps each error variant to an appropriate HTTP status code and JSON
    /// error message in the format: `{"error": "message"}`.
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::Internal(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Internal error: {e}"))
            }
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    /// Converts any `anyhow::Error` into an `ApiError::Internal`.
    ///
    /// This allows using `?` operator with operations in handlers.
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err)
    }
}
