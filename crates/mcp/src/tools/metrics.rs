//! Curated Prometheus metrics queries via the monitor service.
//!
//! The monitor service owns the named PromQL catalog (`GET /queries`) and
//! executes queries against the agentd Prometheus stack. This tool renders
//! the results for MCP clients — instant vectors as a table, range matrices
//! as per-series summaries (never raw sample dumps; token discipline).
//!
//! `get_prometheus_metrics` remains the direct-scrape path that works even
//! when Prometheus itself is down; this tool is the aggregated/historical
//! path.

use crate::client::AgentdClient;
use serde_json::Value;

/// Run a named metrics query, or render the catalog when no/unknown name.
pub async fn run_query_metrics(
    client: &AgentdClient,
    name: Option<&str>,
    window: Option<&str>,
    range: bool,
) -> String {
    let monitor = client.monitor_url();

    let Some(name) = name.filter(|n| !n.trim().is_empty()) else {
        return render_catalog(client).await;
    };

    let mut url = format!("{monitor}/queries/{name}");
    let mut params = vec![];
    if let Some(window) = window {
        params.push(format!("window={window}"));
    }
    if range {
        params.push("mode=range".to_string());
    }
    if !params.is_empty() {
        url = format!("{url}?{}", params.join("&"));
    }

    let response = match client.inner.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return format!(
                "🔴 Monitor service unreachable: {e}\n\
                 → Check it with `check_single_service(\"monitor\")`."
            );
        }
    };

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if status.as_u16() == 502 {
        return format!(
            "🟡 Prometheus is unreachable (monitor returned 502).\n\
             Is the observability stack running? On macOS: \
             `launchctl list | grep com.agentd.prometheus`.\n\
             → Fallback: `get_prometheus_metrics(<service>)` scrapes services \
             directly without Prometheus.\n\n\
             Details: {body}"
        );
    }
    if !status.is_success() {
        // 404 includes the catalog names; 400 explains the bad parameter.
        return format!("🔴 Query `{name}` failed (HTTP {}): {body}", status.as_u16());
    }

    match serde_json::from_str::<Value>(&body) {
        Ok(result) => render_query_result(name, &result),
        Err(e) => format!("🔴 Failed to parse monitor response for `{name}`: {e}"),
    }
}

/// Render the catalog fetched from `GET {monitor}/queries`.
async fn render_catalog(client: &AgentdClient) -> String {
    let url = format!("{}/queries", client.monitor_url());
    let catalog: Value = match client.get(&url).await {
        Ok(v) => v,
        Err(e) => {
            return format!(
                "🔴 Could not fetch the query catalog from the monitor service: {e}\n\
                 → Check it with `check_single_service(\"monitor\")`."
            );
        }
    };

    let Some(entries) = catalog.as_array() else {
        return "🔴 Unexpected catalog shape from the monitor service.".to_string();
    };

    let mut out = String::from(
        "## Metrics Query Catalog\n\n\
         Call `query_metrics(name, window?, range?)` with one of:\n\n\
         | Name | Unit | Description |\n|---|---|---|\n",
    );
    for entry in entries {
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            entry["name"].as_str().unwrap_or("?"),
            entry["unit"].as_str().unwrap_or("?"),
            entry["description"].as_str().unwrap_or(""),
        ));
    }
    out.push_str(
        "\nWindows look like `15m`, `1h`, `24h`. Pass `range: true` for a \
         six-hour trend (per-series min/avg/max/last) instead of an instant value.",
    );
    out
}

/// Render a monitor QueryResult JSON as markdown.
fn render_query_result(name: &str, result: &Value) -> String {
    let promql = result["promql"].as_str().unwrap_or("?");
    let mode = result["mode"].as_str().unwrap_or("instant");
    let data = &result["data"];

    let mut out = format!("## Query `{name}` ({mode})\n\n`{promql}`\n\n");

    match data["resultType"].as_str() {
        Some("vector") => {
            let samples = data["result"].as_array().cloned().unwrap_or_default();
            if samples.is_empty() {
                out.push_str("_No data — the underlying series have no samples (yet)._");
                return out;
            }
            out.push_str("| Labels | Value |\n|---|---|\n");
            for sample in &samples {
                let labels = render_labels(&sample["metric"]);
                let value = sample["value"][1].as_str().unwrap_or("?");
                out.push_str(&format!("| {labels} | {value} |\n"));
            }
        }
        Some("matrix") => {
            let series = data["result"].as_array().cloned().unwrap_or_default();
            if series.is_empty() {
                out.push_str("_No data in the requested range._");
                return out;
            }
            out.push_str("| Labels | Min | Avg | Max | Last |\n|---|---|---|---|---|\n");
            for s in &series {
                let labels = render_labels(&s["metric"]);
                let values: Vec<f64> = s["values"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|pair| pair[1].as_str().and_then(|v| v.parse().ok()))
                    .collect();
                if values.is_empty() {
                    continue;
                }
                let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                let last = values.last().copied().unwrap_or(f64::NAN);
                out.push_str(&format!(
                    "| {labels} | {min:.4} | {avg:.4} | {max:.4} | {last:.4} |\n"
                ));
            }
        }
        Some("scalar") => {
            let value = data["result"][1].as_str().unwrap_or("?");
            out.push_str(&format!("**Value:** {value}"));
        }
        other => {
            out.push_str(&format!("_Unrecognized result type: {other:?}_"));
        }
    }

    out
}

/// Render a Prometheus label map compactly, skipping `__name__`.
fn render_labels(metric: &Value) -> String {
    let Some(map) = metric.as_object() else { return "—".to_string() };
    let rendered: Vec<String> = map
        .iter()
        .filter(|(k, _)| *k != "__name__")
        .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("?")))
        .collect();
    if rendered.is_empty() {
        "—".to_string()
    } else {
        rendered.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn vector_result_renders_label_value_table() {
        let result = json!({
            "name": "agents-active",
            "promql": "agents_active",
            "mode": "instant",
            "data": {
                "resultType": "vector",
                "result": [
                    {"metric": {"__name__": "agents_active", "service": "orchestrator"},
                     "value": [1718100000.0, "3"]}
                ]
            }
        });
        let out = render_query_result("agents-active", &result);
        assert!(out.contains("| service=orchestrator | 3 |"), "{out}");
        assert!(!out.contains("__name__"), "name label hidden: {out}");
    }

    #[test]
    fn matrix_result_renders_summary_not_samples() {
        let result = json!({
            "promql": "x",
            "mode": "range",
            "data": {
                "resultType": "matrix",
                "result": [
                    {"metric": {"service": "notify"},
                     "values": [[1.0, "1"], [2.0, "3"], [3.0, "2"]]}
                ]
            }
        });
        let out = render_query_result("test", &result);
        assert!(out.contains("| Min | Avg | Max | Last |"), "{out}");
        assert!(out.contains("1.0000"), "min: {out}");
        assert!(out.contains("3.0000"), "max: {out}");
        assert!(out.contains("2.0000"), "last/avg: {out}");
        assert!(!out.contains("[[1.0"), "no raw sample dumps: {out}");
    }

    #[test]
    fn empty_vector_renders_no_data_note() {
        let result = json!({
            "promql": "x", "mode": "instant",
            "data": { "resultType": "vector", "result": [] }
        });
        let out = render_query_result("test", &result);
        assert!(out.contains("No data"), "{out}");
    }

    #[test]
    fn scalar_result_renders_value() {
        let result = json!({
            "promql": "scalar(1)", "mode": "instant",
            "data": { "resultType": "scalar", "result": [1718100000.0, "42"] }
        });
        let out = render_query_result("test", &result);
        assert!(out.contains("**Value:** 42"), "{out}");
    }
}
