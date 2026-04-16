//! API gateway proxy handlers.
//!
//! Routes incoming requests under `/api/v1/{service}/*` to the corresponding
//! downstream agentd service. Each proxied request has:
//!
//! - `X-Tenant-ID` injected from the authenticated user's active organization
//! - `X-Request-ID` injected (new UUID per request)
//! - `Authorization` header forwarded to the downstream service
//!
//! # Route Mapping
//!
//! | Incoming path                       | Downstream service        |
//! |-------------------------------------|---------------------------|
//! | `/api/v1/orchestrator/*`            | agentd-orchestrator       |
//! | `/api/v1/notify/*`                  | agentd-notify             |
//! | `/api/v1/ask/*`                     | agentd-ask                |
//! | `/api/v1/wrap/*`                    | agentd-wrap               |
//! | `/api/v1/hook/*`                    | agentd-hook               |
//! | `/api/v1/monitor/*`                 | agentd-monitor            |
//!
//! # Health aggregation
//!
//! `GET /api/v1/health` checks all downstream services concurrently and
//! returns a summary. Downstream unavailability yields `503 Service Unavailable`.

use axum::{
    body::Body,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    middleware::tenant::TenantContext,
    proxy::{health_check, proxy_request, ProxyConfig, ProxyRequest},
};

use super::AppState;

// ---------------------------------------------------------------------------
// State extension
// ---------------------------------------------------------------------------

/// Gateway-specific state carried alongside AppState.
///
/// Mounted via `Router::with_state(GatewayState { ... })` on the gateway
/// sub-router and extracted with `State<GatewayState>`.
#[derive(Clone)]
pub struct GatewayState {
    pub app: AppState,
    pub proxy: ProxyConfig,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(proxy: ProxyConfig) -> Router<AppState> {
    // Build a nested router that merges AppState with ProxyConfig into GatewayState.
    // We use a closure-based approach: each handler receives both State<AppState>
    // and the proxy config via a separate extension.
    //
    // Axum requires a single state type per router, so we wrap both into GatewayState
    // using `Router::with_state` at the gateway level.
    let proxy_clone = proxy.clone();

    Router::new()
        .route(
            "/health",
            get(move |state: State<AppState>| {
                let p = proxy_clone.clone();
                async move { health_handler(state, p).await }
            }),
        )
        .route(
            "/{service}/{*path}",
            axum::routing::any(proxy_handler).layer(axum::Extension(proxy)),
        )
}

// ---------------------------------------------------------------------------
// Health aggregation
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ServiceHealth {
    name: String,
    url: String,
    healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    services: Vec<ServiceHealth>,
}

async fn health_handler(State(state): State<AppState>, proxy: ProxyConfig) -> impl IntoResponse {
    let _ = state; // AppState not used for health — kept for router consistency

    let mut handles = Vec::new();

    for (&name, url) in &proxy.services {
        let client = proxy.client.clone();
        let url = url.clone();
        let name = name.to_string();
        handles.push(tokio::spawn(async move {
            let (healthy, detail) = health_check(&client, &url).await;
            ServiceHealth { name, url, healthy, detail }
        }));
    }

    let mut services = Vec::new();
    for handle in handles {
        if let Ok(s) = handle.await {
            services.push(s);
        }
    }
    services.sort_by(|a, b| a.name.cmp(&b.name));

    let all_healthy = services.iter().all(|s| s.healthy);
    let status = if all_healthy { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };

    (status, Json(HealthResponse { status: if all_healthy { "ok" } else { "degraded" }, services }))
}

// ---------------------------------------------------------------------------
// Proxy handler
// ---------------------------------------------------------------------------

async fn proxy_handler(
    State(state): State<AppState>,
    axum::Extension(proxy): axum::Extension<ProxyConfig>,
    tenant: TenantContext,
    Path((service, path)): Path<(String, String)>,
    Query(query_params): Query<HashMap<String, String>>,
    req: Request,
) -> Response {
    // Resolve the downstream URL
    let base_url = match proxy.url_for(&service) {
        Some(u) => u.to_string(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": format!("unknown service: {service}")
                })),
            )
                .into_response();
        }
    };

    let method = req.method().as_str().to_string();

    // Build query string
    let query_string = if query_params.is_empty() {
        None
    } else {
        Some(
            query_params
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding_simple(k), urlencoding_simple(v)))
                .collect::<Vec<_>>()
                .join("&"),
        )
    };

    // Collect headers to forward
    let headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value.to_str().ok().map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect();

    // Read body bytes
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("failed to read request body: {e}") })),
            )
                .into_response();
        }
    };

    // Generate a request ID
    let request_id = Uuid::new_v4().to_string();
    let forward_path = format!("/{path}");

    match proxy_request(
        &proxy.client,
        &base_url,
        ProxyRequest {
            method: &method,
            path: &forward_path,
            query: query_string.as_deref(),
            headers: &headers,
            body: body_bytes,
            tenant_id: &tenant.organization_id,
            request_id: &request_id,
        },
    )
    .await
    {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let mut response_headers = HeaderMap::new();
            for (name, value) in &resp.headers {
                if let (Ok(hn), Ok(hv)) = (
                    axum::http::HeaderName::from_bytes(name.as_bytes()),
                    axum::http::HeaderValue::from_str(value),
                ) {
                    response_headers.insert(hn, hv);
                }
            }
            (status, response_headers, Body::from(resp.body)).into_response()
        }
        Err(e) => {
            let _ = state; // suppress unused warning
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": format!("upstream error: {e}")
                })),
            )
                .into_response()
        }
    }
}

/// Minimal percent-encoding for query values (encodes spaces and `&`/`=`).
fn urlencoding_simple(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                vec![c]
            }
            ' ' => vec!['+'],
            c => {
                let mut buf = [0u8; 4];
                let bytes = c.encode_utf8(&mut buf);
                bytes
                    .bytes()
                    .flat_map(|b| {
                        vec![
                            '%',
                            char::from_digit((b >> 4) as u32, 16).unwrap_or('0'),
                            char::from_digit((b & 0xf) as u32, 16).unwrap_or('0'),
                        ]
                    })
                    .collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urlencoding_simple() {
        assert_eq!(urlencoding_simple("hello world"), "hello+world");
        assert_eq!(urlencoding_simple("abc123"), "abc123");
        assert_eq!(urlencoding_simple("a&b=c"), "a%26b%3dc");
    }
}
