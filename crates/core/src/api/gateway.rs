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
//! | `/api/v1/memory/*`                  | agentd-memory             |
//! | `/api/v1/communicate/*`             | agentd-communicate        |
//! | `/api/v1/knowledge/*`               | agentd-knowledge          |
//!
//! # WebSocket proxying
//!
//! Upgrade requests (e.g. `GET /api/v1/orchestrator/stream`) are detected by
//! the `Upgrade: websocket` header and bridged to the downstream service over a
//! client WebSocket connection. Because browsers cannot set request headers on
//! a WebSocket handshake, these requests authenticate from a `token` query
//! parameter rather than the `Authorization` header; the resolved tenant is
//! still injected downstream as `X-Tenant-ID`.
//!
//! # Health aggregation
//!
//! `GET /api/v1/health` checks all downstream services concurrently and
//! returns a summary. Downstream unavailability yields `503 Service Unavailable`.

use axum::{
    body::Body,
    extract::{
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
        FromRequestParts, Path, Query, Request, State,
    },
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest, protocol::CloseFrame as TungCloseFrame, Message as TungMessage,
    },
    MaybeTlsStream, WebSocketStream,
};
use tracing::warn;
use uuid::Uuid;

use crate::{
    middleware::tenant::{TenantContext, TenantError},
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

    let request_id = Uuid::new_v4().to_string();

    // WebSocket upgrades take a separate path: browsers cannot set the
    // Authorization header on a handshake, so we authenticate from the `token`
    // query parameter and bridge the two sockets instead of buffering bytes.
    if is_websocket_upgrade(req.headers()) {
        let tenant = match TenantContext::resolve(
            &state,
            query_params.get("token").cloned().unwrap_or_default(),
        )
        .await
        {
            Ok(t) => t,
            Err(e) => return e.into_response(),
        };
        return proxy_websocket(state, req, base_url, &path, &query_params, &tenant, &request_id)
            .await;
    }

    // HTTP requests authenticate from the Authorization header (the standard
    // TenantContext extractor logic, applied manually so WS can opt out).
    let tenant = match resolve_tenant_from_header(&state, req.headers()).await {
        Ok(t) => t,
        Err(e) => return e.into_response(),
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
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": format!("upstream error: {e}")
            })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// WebSocket proxy
// ---------------------------------------------------------------------------

/// Returns true when the request is a WebSocket upgrade handshake.
fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

/// Resolve a [`TenantContext`] from the `Authorization: Bearer` header.
///
/// Mirrors the [`TenantContext`] extractor, but applied manually so the
/// WebSocket path can authenticate from a query parameter instead.
async fn resolve_tenant_from_header(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, TenantError> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or(TenantError::MissingToken)?;
    TenantContext::resolve(state, token).await
}

/// Proxy a WebSocket connection to a downstream service.
///
/// Accepts the client-side upgrade, dials the upstream service over WebSocket
/// (injecting `X-Tenant-ID` / `X-Request-ID`), and bridges messages in both
/// directions until either side closes.
async fn proxy_websocket(
    state: AppState,
    req: Request,
    base_url: String,
    path: &str,
    query_params: &HashMap<String, String>,
    tenant: &TenantContext,
    request_id: &str,
) -> Response {
    // Build the upstream WebSocket URL (http→ws, https→wss), forwarding the
    // original query string minus the `token` credential we consumed for auth.
    let scheme = if base_url.starts_with("https://") { "wss" } else { "ws" };
    let authority = base_url.split_once("://").map(|(_, rest)| rest).unwrap_or(base_url.as_str());
    let mut upstream_url = format!("{scheme}://{authority}/{path}");
    let forwarded_query = query_params
        .iter()
        .filter(|(k, _)| k.as_str() != "token")
        .map(|(k, v)| format!("{}={}", urlencoding_simple(k), urlencoding_simple(v)))
        .collect::<Vec<_>>()
        .join("&");
    if !forwarded_query.is_empty() {
        upstream_url.push('?');
        upstream_url.push_str(&forwarded_query);
    }

    let tenant_id = tenant.organization_id.clone();
    let request_id = request_id.to_string();

    // Accept the client-side upgrade.
    let (mut parts, _body) = req.into_parts();
    let ws = match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
        Ok(ws) => ws,
        Err(rej) => return rej.into_response(),
    };

    ws.on_upgrade(move |client| async move {
        // Build the upstream handshake request with the standard WS headers
        // (filled in by into_client_request) plus our injected gateway headers.
        let mut upstream_req = match upstream_url.as_str().into_client_request() {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, url = %upstream_url, "gateway ws: invalid upstream url");
                return;
            }
        };
        let req_headers = upstream_req.headers_mut();
        if let Ok(v) = HeaderValue::from_str(&tenant_id) {
            req_headers.insert("X-Tenant-ID", v);
        }
        if let Ok(v) = HeaderValue::from_str(&request_id) {
            req_headers.insert("X-Request-ID", v);
        }

        let (upstream, _resp) = match connect_async(upstream_req).await {
            Ok(pair) => pair,
            Err(e) => {
                warn!(error = %e, url = %upstream_url, "gateway ws: upstream connect failed");
                return;
            }
        };

        bridge_sockets(client, upstream).await;
    })
    .into_response()
}

/// Bridge an accepted client socket with an upstream connection, forwarding
/// messages in both directions until either side closes or errors.
async fn bridge_sockets(client: WebSocket, upstream: WebSocketStream<MaybeTlsStream<TcpStream>>) {
    let (mut client_tx, mut client_rx) = client.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    // Client → upstream
    let mut c2u = tokio::spawn(async move {
        while let Some(Ok(msg)) = client_rx.next().await {
            let is_close = matches!(msg, AxumMessage::Close(_));
            if upstream_tx.send(axum_to_tungstenite(msg)).await.is_err() {
                break;
            }
            if is_close {
                break;
            }
        }
    });

    // Upstream → client
    let mut u2c = tokio::spawn(async move {
        while let Some(Ok(msg)) = upstream_rx.next().await {
            // Tungstenite raw frames have no axum equivalent; skip them.
            let Some(msg) = tungstenite_to_axum(msg) else { continue };
            let is_close = matches!(msg, AxumMessage::Close(_));
            if client_tx.send(msg).await.is_err() {
                break;
            }
            if is_close {
                break;
            }
        }
    });

    // When either direction finishes, tear down the other.
    tokio::select! {
        _ = &mut c2u => u2c.abort(),
        _ = &mut u2c => c2u.abort(),
    }
}

/// Convert an axum WebSocket message into its tungstenite equivalent.
fn axum_to_tungstenite(msg: AxumMessage) -> TungMessage {
    match msg {
        AxumMessage::Text(t) => TungMessage::Text(t.as_str().into()),
        AxumMessage::Binary(b) => TungMessage::Binary(b),
        AxumMessage::Ping(b) => TungMessage::Ping(b),
        AxumMessage::Pong(b) => TungMessage::Pong(b),
        AxumMessage::Close(frame) => TungMessage::Close(
            frame.map(|f| TungCloseFrame { code: f.code.into(), reason: f.reason.as_str().into() }),
        ),
    }
}

/// Convert a tungstenite WebSocket message into its axum equivalent.
///
/// Returns `None` for raw `Frame` messages, which have no axum counterpart.
fn tungstenite_to_axum(msg: TungMessage) -> Option<AxumMessage> {
    Some(match msg {
        TungMessage::Text(t) => AxumMessage::Text(t.as_str().into()),
        TungMessage::Binary(b) => AxumMessage::Binary(b),
        TungMessage::Ping(b) => AxumMessage::Ping(b),
        TungMessage::Pong(b) => AxumMessage::Pong(b),
        TungMessage::Close(frame) => AxumMessage::Close(frame.map(|f| {
            axum::extract::ws::CloseFrame { code: f.code.into(), reason: f.reason.as_str().into() }
        })),
        TungMessage::Frame(_) => return None,
    })
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

    #[test]
    fn test_is_websocket_upgrade() {
        let mut headers = HeaderMap::new();
        assert!(!is_websocket_upgrade(&headers));
        headers.insert(axum::http::header::UPGRADE, HeaderValue::from_static("websocket"));
        assert!(is_websocket_upgrade(&headers));
        // Case-insensitive per RFC 6455.
        headers.insert(axum::http::header::UPGRADE, HeaderValue::from_static("WebSocket"));
        assert!(is_websocket_upgrade(&headers));
        headers.insert(axum::http::header::UPGRADE, HeaderValue::from_static("h2c"));
        assert!(!is_websocket_upgrade(&headers));
    }

    #[test]
    fn test_message_conversion_roundtrip() {
        // Text
        let a = AxumMessage::Text("hello".into());
        match axum_to_tungstenite(a) {
            TungMessage::Text(t) => assert_eq!(t.as_str(), "hello"),
            other => panic!("expected text, got {other:?}"),
        }
        match tungstenite_to_axum(TungMessage::Text("world".into())) {
            Some(AxumMessage::Text(t)) => assert_eq!(t.as_str(), "world"),
            other => panic!("expected text, got {other:?}"),
        }

        // Binary
        match axum_to_tungstenite(AxumMessage::Binary(vec![1, 2, 3].into())) {
            TungMessage::Binary(b) => assert_eq!(&b[..], &[1, 2, 3]),
            other => panic!("expected binary, got {other:?}"),
        }

        // Close frame preserves code + reason
        let close = AxumMessage::Close(Some(axum::extract::ws::CloseFrame {
            code: 1000,
            reason: "bye".into(),
        }));
        match axum_to_tungstenite(close) {
            TungMessage::Close(Some(f)) => {
                assert_eq!(u16::from(f.code), 1000);
                assert_eq!(f.reason.as_str(), "bye");
            }
            other => panic!("expected close, got {other:?}"),
        }

        // Raw frames have no axum equivalent.
        assert!(tungstenite_to_axum(TungMessage::Ping(vec![].into())).is_some());
    }
}
