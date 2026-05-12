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
/// Reads `RUST_LOG` for the log filter (defaults to `info`) and `AGENTD_LOG_FORMAT`
/// for the output format (`json` for structured JSON, anything else for
/// human-readable text).
///
/// This function should be called once at the start of each service binary.
///
/// # Environment Variables
///
/// - `RUST_LOG` — Controls log level/filter (e.g., `debug`, `info`, `warn`)
/// - `AGENTD_LOG_FORMAT` — Set to `json` for structured JSON log output
pub fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if std::env::var("AGENTD_LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt().json().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }
}

/// Create the standard TraceLayer middleware for HTTP request/response logging.
///
/// Returns a configured `TraceLayer` that logs requests and responses at INFO level.
/// Used by all agentd services for consistent HTTP observability.
pub fn trace_layer() -> tower_http::trace::TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
> {
    tower_http::trace::TraceLayer::new_for_http()
        .make_span_with(tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO))
        .on_response(tower_http::trace::DefaultOnResponse::new().level(tracing::Level::INFO))
}

/// Create a CORS layer configured from the environment.
///
/// Reads the `AGENTD_CORS_ORIGINS` environment variable to determine allowed origins.
/// Defaults to `*` (any origin) when the variable is not set, which is appropriate
/// for local development. Set to a comma-separated list of origins for production.
///
/// # Allowed Configuration
///
/// - **Methods**: GET, POST, PUT, DELETE, OPTIONS
/// - **Headers**: Content-Type, Authorization, and WebSocket upgrade headers
/// - **Origins**: Configurable via `AGENTD_CORS_ORIGINS` env var (default: `*`)
///
/// # Environment Variables
///
/// - `AGENTD_CORS_ORIGINS` — Comma-separated list of allowed origins, or `*` for any.
///   Example: `https://app.example.com,https://admin.example.com`
///
/// # Examples
///
/// ```rust,ignore
/// use agentd_common::server::cors_layer;
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(cors_layer());
/// ```
#[cfg(test)]
mod tests {
    // --- trace_layer ---

    #[test]
    fn test_trace_layer_constructs_without_panic() {
        // Verifies that trace_layer() can be called without panicking.
        // We cannot easily inspect TraceLayer internals; construction smoke
        // test is sufficient to catch configuration regressions.
        let _layer = super::trace_layer();
    }

    // --- cors_layer ---
    //
    // All AGENTD_CORS_ORIGINS variants are tested inside a single function to
    // prevent env-var races across parallel test threads.

    #[test]
    fn test_cors_layer_respects_agentd_cors_origins() {
        // default (no env var) — should not panic; defaults to AllowOrigin::any()
        std::env::remove_var("AGENTD_CORS_ORIGINS");
        let _layer = super::cors_layer();

        // explicit wildcard
        std::env::set_var("AGENTD_CORS_ORIGINS", "*");
        let _layer = super::cors_layer();

        // single origin
        std::env::set_var("AGENTD_CORS_ORIGINS", "https://app.example.com");
        let _layer = super::cors_layer();

        // multiple origins
        std::env::set_var(
            "AGENTD_CORS_ORIGINS",
            "https://app.example.com,https://admin.example.com",
        );
        let _layer = super::cors_layer();

        // whitespace around commas should be trimmed without panic
        std::env::set_var(
            "AGENTD_CORS_ORIGINS",
            "https://app.example.com , https://admin.example.com",
        );
        let _layer = super::cors_layer();

        std::env::remove_var("AGENTD_CORS_ORIGINS");
    }
}

pub fn cors_layer() -> tower_http::cors::CorsLayer {
    use axum::http::{header, HeaderName, HeaderValue, Method};
    use tower_http::cors::{AllowOrigin, CorsLayer};

    let origins = std::env::var("AGENTD_CORS_ORIGINS").unwrap_or_else(|_| "*".to_string());

    let allow_origin = if origins.trim() == "*" {
        AllowOrigin::any()
    } else {
        let values: Vec<HeaderValue> =
            origins.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        AllowOrigin::list(values)
    };

    CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            // WebSocket upgrade headers
            header::UPGRADE,
            header::CONNECTION,
            HeaderName::from_static("sec-websocket-key"),
            HeaderName::from_static("sec-websocket-version"),
            HeaderName::from_static("sec-websocket-protocol"),
            HeaderName::from_static("sec-websocket-extensions"),
        ])
}
