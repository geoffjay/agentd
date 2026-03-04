//! Server initialization and tracing setup helpers.
//!
//! Shared utilities for starting agentd service binaries with consistent
//! tracing configuration and HTTP middleware.
//!
//! # Examples
//!
//! ```rust,ignore
//! use agentd_common::server::init_tracing;
//!
//! #[tokio::main]
//! async fn main() {
//!     init_tracing();
//!     tracing::info!("Service starting...");
//! }
//! ```

/// Initialize the tracing subscriber with environment-based configuration.
///
/// Reads `RUST_LOG` for the log filter (defaults to `info`) and `LOG_FORMAT`
/// for the output format (`json` for structured JSON, anything else for
/// human-readable text).
///
/// This function should be called once at the start of each service binary.
///
/// # Environment Variables
///
/// - `RUST_LOG` — Controls log level/filter (e.g., `debug`, `info`, `warn`)
/// - `LOG_FORMAT` — Set to `json` for structured JSON log output
pub fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if std::env::var("LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }
}

/// Create the standard TraceLayer middleware for HTTP request/response logging.
///
/// Returns a configured `TraceLayer` that logs requests and responses at INFO level.
/// Used by all agentd services for consistent HTTP observability.
pub fn trace_layer() -> tower_http::trace::TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>> {
    tower_http::trace::TraceLayer::new_for_http()
        .make_span_with(
            tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO),
        )
        .on_response(
            tower_http::trace::DefaultOnResponse::new().level(tracing::Level::INFO),
        )
}
