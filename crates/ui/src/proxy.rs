use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use reqwest::Client;

/// Shared state for proxy handlers.
///
/// After the React SPA was updated to route directly through the core gateway,
/// the per-service proxy handlers became redundant. The proxy now forwards all
/// `/api/**` requests to the core gateway at `{gateway_url}/api/v1/**`, so
/// only a single upstream URL is needed.
#[derive(Clone)]
pub struct ProxyState {
    pub client: Client,
    /// URL of the core gateway (e.g. `http://localhost:17000`).
    pub gateway_url: String,
}

/// Proxy all requests under `/api/**` to the core gateway.
///
/// Strips the `/api/` prefix and prepends `/api/v1/` so that the gateway
/// receives the correct service path.
///
/// Example: `GET /api/notify/notifications` → `GET {gateway}/api/v1/notify/notifications`
pub async fn proxy_to_gateway(
    State(state): State<ProxyState>,
    req: Request<Body>,
) -> Result<Response, StatusCode> {
    proxy_request(&state.client, &state.gateway_url, "/api", "/api/v1", req).await
}

/// Forward an inbound request to the upstream gateway, rewriting the path prefix.
async fn proxy_request(
    client: &Client,
    upstream_base: &str,
    strip_prefix: &str,
    add_prefix: &str,
    req: Request<Body>,
) -> Result<Response, StatusCode> {
    let tail = req.uri().path().strip_prefix(strip_prefix).unwrap_or("");
    let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let target_url = format!("{upstream_base}{add_prefix}{tail}{query}");

    let method = req.method().clone();
    let headers = req.headers().clone();
    let body_bytes =
        axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await.map_err(|_| {
            tracing::error!("Failed to read request body");
            StatusCode::BAD_REQUEST
        })?;

    let mut upstream_req = client.request(method, &target_url);
    for (key, value) in headers.iter() {
        if key != "host" {
            upstream_req = upstream_req.header(key, value);
        }
    }
    upstream_req = upstream_req.body(body_bytes);

    let upstream_resp = upstream_req.send().await.map_err(|e| {
        tracing::error!("Proxy request to {} failed: {}", target_url, e);
        StatusCode::BAD_GATEWAY
    })?;

    let status = StatusCode::from_u16(upstream_resp.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let resp_headers = upstream_resp.headers().clone();
    let resp_body = upstream_resp.bytes().await.map_err(|e| {
        tracing::error!("Failed to read upstream response: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let mut response = (status, resp_body).into_response();
    for (key, value) in resp_headers.iter() {
        response.headers_mut().insert(key, value.clone());
    }

    Ok(response)
}
