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

/// Middleware layer that records HTTP request counts and durations as Prometheus metrics.
///
/// Records two metrics for every request:
/// - `http_requests_total` (counter) with labels: `method`, `path`, `status`
/// - `http_request_duration_seconds` (histogram) with labels: `method`, `path`
///
/// Paths are normalized to avoid high cardinality: UUID segments are replaced
/// with `{id}` (e.g., `/agents/550e8400-…/message` → `/agents/{id}/message`).
pub fn metrics_layer() -> MetricsLayer {
    MetricsLayer
}

/// Tower layer that wraps services with HTTP metrics recording.
#[derive(Clone, Copy)]
pub struct MetricsLayer;

impl<S> tower::Layer<S> for MetricsLayer {
    type Service = MetricsMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsMiddleware { inner }
    }
}

/// Tower service that records HTTP request metrics.
#[derive(Clone)]
pub struct MetricsMiddleware<S> {
    inner: S,
}

impl<S> tower::Service<axum::http::Request<axum::body::Body>> for MetricsMiddleware<S>
where
    S: tower::Service<axum::http::Request<axum::body::Body>, Response = axum::response::Response>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::http::Request<axum::body::Body>) -> Self::Future {
        let method = req.method().to_string();
        let path = normalize_path(req.uri().path());

        let mut inner = self.inner.clone();
        Box::pin(async move {
            let start = std::time::Instant::now();
            let response = inner.call(req).await?;
            let duration = start.elapsed().as_secs_f64();
            let status = response.status().as_u16().to_string();

            metrics::counter!("http_requests_total",
                "method" => method.clone(),
                "path" => path.clone(),
                "status" => status
            )
            .increment(1);

            metrics::histogram!("http_request_duration_seconds",
                "method" => method,
                "path" => path
            )
            .record(duration);

            Ok(response)
        })
    }
}

/// Replace UUID path segments with `{id}` to keep metric cardinality bounded.
fn normalize_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            // Match UUID v4 patterns (8-4-4-4-12 hex) and bare hex IDs (>= 8 chars)
            if segment.len() >= 8
                && segment.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
                && segment.chars().any(|c| c.is_ascii_digit())
            {
                "{id}"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Create a CORS layer configured from the environment.
///
/// Reads the `AGENTD_CORS_ORIGINS` environment variable to determine allowed origins.
/// Defaults to `*` (any origin) when the variable is not set, which is appropriate
/// for local development. Set to a comma-separated list of origins for production.
///
/// # Allowed Configuration
///
/// - **Methods**: GET, POST, PUT, PATCH, DELETE, OPTIONS
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
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
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
