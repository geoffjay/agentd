//! Minimal Prometheus HTTP API client.
//!
//! Hand-rolled over `reqwest` rather than pulling in a query-client crate:
//! only two endpoints are needed (`/api/v1/query` and `/api/v1/query_range`),
//! and the workspace convention is thin typed reqwest wrappers.
//!
//! The agentd observability stack runs Prometheus at `127.0.0.1:9090`
//! (see `infra/prometheus/` and the launchd plist) scraping every service's
//! metrics endpoint.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// Errors from the Prometheus HTTP API.
#[derive(Debug, thiserror::Error)]
pub enum PromError {
    /// Prometheus could not be reached at all.
    #[error("Prometheus unreachable at {url}: {source}")]
    Unreachable {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    /// Prometheus answered with a non-success HTTP status.
    #[error("Prometheus returned HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },
    /// Prometheus accepted the request but rejected the query.
    #[error("Prometheus query error ({error_type}): {error}")]
    Query { error_type: String, error: String },
    /// The response body did not match the expected envelope.
    #[error("Failed to parse Prometheus response: {0}")]
    Parse(String),
}

/// One instant-vector sample: label set + (timestamp, value) pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSample {
    /// Label name → value, including `__name__` when present.
    pub metric: BTreeMap<String, String>,
    /// `[unix_seconds, "value"]` as returned by Prometheus.
    pub value: (f64, String),
}

/// One range-vector series: label set + list of (timestamp, value) pairs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixSeries {
    pub metric: BTreeMap<String, String>,
    /// `[[unix_seconds, "value"], ...]` as returned by Prometheus.
    pub values: Vec<(f64, String)>,
}

/// Query result data, tagged by Prometheus's `resultType`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "resultType", content = "result", rename_all = "lowercase")]
pub enum PromData {
    Vector(Vec<VectorSample>),
    Matrix(Vec<MatrixSeries>),
    Scalar((f64, String)),
}

/// The standard Prometheus HTTP API response envelope.
#[derive(Debug, Deserialize)]
struct PromEnvelope {
    status: String,
    #[serde(default)]
    data: Option<PromData>,
    #[serde(default, rename = "errorType")]
    error_type: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Typed client for the Prometheus HTTP API.
#[derive(Debug, Clone)]
pub struct PromClient {
    http: reqwest::Client,
    base_url: String,
}

impl PromClient {
    /// Create a client for `base_url` (e.g. `"http://127.0.0.1:9090"`).
    pub fn new(base_url: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build reqwest Client");
        Self { http, base_url: base_url.trim_end_matches('/').to_string() }
    }

    /// The configured Prometheus base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// `GET /api/v1/query` — evaluate an instant query.
    pub async fn query(&self, promql: &str) -> Result<PromData, PromError> {
        let url = format!("{}/api/v1/query", self.base_url);
        self.execute(self.http.get(&url).query(&[("query", promql)]), &url).await
    }

    /// `GET /api/v1/query_range` — evaluate a range query.
    pub async fn query_range(
        &self,
        promql: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        step_secs: u64,
    ) -> Result<PromData, PromError> {
        let url = format!("{}/api/v1/query_range", self.base_url);
        let request = self.http.get(&url).query(&[
            ("query", promql.to_string()),
            ("start", start.timestamp().to_string()),
            ("end", end.timestamp().to_string()),
            ("step", format!("{step_secs}s")),
        ]);
        self.execute(request, &url).await
    }

    async fn execute(
        &self,
        request: reqwest::RequestBuilder,
        url: &str,
    ) -> Result<PromData, PromError> {
        let response = request
            .send()
            .await
            .map_err(|source| PromError::Unreachable { url: url.to_string(), source })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|source| PromError::Unreachable { url: url.to_string(), source })?;

        // Prometheus returns query errors with non-2xx statuses AND a JSON
        // envelope; prefer the envelope's message when it parses.
        let envelope: Result<PromEnvelope, _> = serde_json::from_str(&body);
        match envelope {
            Ok(env) if env.status == "success" => {
                env.data.ok_or_else(|| PromError::Parse("missing data field".to_string()))
            }
            Ok(env) => Err(PromError::Query {
                error_type: env.error_type.unwrap_or_else(|| "unknown".to_string()),
                error: env.error.unwrap_or_else(|| "unknown error".to_string()),
            }),
            Err(_) if !status.is_success() => {
                Err(PromError::HttpStatus { status: status.as_u16(), body })
            }
            Err(e) => Err(PromError::Parse(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_envelope_deserializes() {
        let body = r#"{
            "status": "success",
            "data": {
                "resultType": "vector",
                "result": [
                    {"metric": {"__name__": "agents_active", "service": "orchestrator"},
                     "value": [1718100000.0, "3"]}
                ]
            }
        }"#;
        let env: PromEnvelope = serde_json::from_str(body).unwrap();
        assert_eq!(env.status, "success");
        match env.data.unwrap() {
            PromData::Vector(samples) => {
                assert_eq!(samples.len(), 1);
                assert_eq!(samples[0].metric["service"], "orchestrator");
                assert_eq!(samples[0].value.1, "3");
            }
            other => panic!("expected vector, got {other:?}"),
        }
    }

    #[test]
    fn matrix_envelope_deserializes() {
        let body = r#"{
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {"metric": {"service": "notify"},
                     "values": [[1718100000.0, "1"], [1718100060.0, "2"]]}
                ]
            }
        }"#;
        let env: PromEnvelope = serde_json::from_str(body).unwrap();
        match env.data.unwrap() {
            PromData::Matrix(series) => {
                assert_eq!(series[0].values.len(), 2);
                assert_eq!(series[0].values[1].1, "2");
            }
            other => panic!("expected matrix, got {other:?}"),
        }
    }

    #[test]
    fn scalar_envelope_deserializes() {
        let body = r#"{
            "status": "success",
            "data": { "resultType": "scalar", "result": [1718100000.0, "42"] }
        }"#;
        let env: PromEnvelope = serde_json::from_str(body).unwrap();
        assert!(matches!(env.data.unwrap(), PromData::Scalar((_, v)) if v == "42"));
    }

    #[test]
    fn error_envelope_maps_to_query_error() {
        let body = r#"{
            "status": "error",
            "errorType": "bad_data",
            "error": "parse error: unexpected character"
        }"#;
        let env: PromEnvelope = serde_json::from_str(body).unwrap();
        assert_eq!(env.status, "error");
        assert_eq!(env.error_type.as_deref(), Some("bad_data"));
    }

    #[test]
    fn base_url_trailing_slash_is_trimmed() {
        let client = PromClient::new("http://127.0.0.1:9090/".to_string());
        assert_eq!(client.base_url(), "http://127.0.0.1:9090");
    }

    #[tokio::test]
    async fn unreachable_prometheus_yields_unreachable_error() {
        let client = PromClient::new("http://127.0.0.1:1".to_string());
        let err = client.query("up").await.unwrap_err();
        assert!(matches!(err, PromError::Unreachable { .. }), "{err}");
    }
}
