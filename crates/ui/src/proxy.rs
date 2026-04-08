use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use reqwest::Client;

/// Shared state for proxy handlers.
#[derive(Clone)]
pub struct ProxyState {
    pub client: Client,
    pub ask_url: String,
    pub notify_url: String,
    pub orchestrator_url: String,
    pub index_url: String,
}

/// Proxy requests under `/api/ask/**` to the ask service.
pub async fn proxy_ask(
    State(state): State<ProxyState>,
    req: Request<Body>,
) -> Result<Response, StatusCode> {
    proxy_request(&state.client, &state.ask_url, "/api/ask", req).await
}

/// Proxy requests under `/api/notify/**` to the notify service.
pub async fn proxy_notify(
    State(state): State<ProxyState>,
    req: Request<Body>,
) -> Result<Response, StatusCode> {
    proxy_request(&state.client, &state.notify_url, "/api/notify", req).await
}

/// Proxy requests under `/api/orchestrator/**` to the orchestrator service.
pub async fn proxy_orchestrator(
    State(state): State<ProxyState>,
    req: Request<Body>,
) -> Result<Response, StatusCode> {
    proxy_request(&state.client, &state.orchestrator_url, "/api/orchestrator", req).await
}

/// Proxy requests under `/api/index/**` to the index service.
pub async fn proxy_index(
    State(state): State<ProxyState>,
    req: Request<Body>,
) -> Result<Response, StatusCode> {
    proxy_request(&state.client, &state.index_url, "/api/index", req).await
}

/// Forward an inbound request to an upstream service, stripping the prefix.
async fn proxy_request(
    client: &Client,
    upstream_url: &str,
    prefix: &str,
    req: Request<Body>,
) -> Result<Response, StatusCode> {
    let path = req.uri().path().strip_prefix(prefix).unwrap_or("");
    let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let target_url = format!("{upstream_url}{path}{query}");

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
