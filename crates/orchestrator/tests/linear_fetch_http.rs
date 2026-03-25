//! Integration tests for `LinearIssueSource::fetch_tasks()` against a mock HTTP server.
//!
//! These tests spin up a local `wiremock` server on a random port and verify
//! that `fetch_tasks()` correctly parses single-page responses, follows
//! pagination cursors, surfaces API errors, and handles empty result sets.
//!
//! # Design
//!
//! `LinearIssueSource::new_with_url()` (a `#[cfg(test)]`-only constructor)
//! accepts an explicit API URL so the source can be pointed at the mock
//! server instead of `https://api.linear.app/graphql`.

use orchestrator::scheduler::linear::LinearIssueSource;
use orchestrator::scheduler::source::TaskSource;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Helper: build a full GraphQL response body
// ---------------------------------------------------------------------------

fn graphql_response(
    nodes: serde_json::Value,
    has_next_page: bool,
    end_cursor: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "data": {
            "issues": {
                "nodes": nodes,
                "pageInfo": {
                    "hasNextPage": has_next_page,
                    "endCursor": end_cursor
                }
            }
        }
    })
}

fn single_issue_node(identifier: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "id": format!("uuid-{}", identifier),
        "identifier": identifier,
        "title": title,
        "description": "Test description",
        "url": format!("https://linear.app/team/issue/{}", identifier),
        "state": { "name": "Todo" },
        "priority": 2,
        "assignee": { "displayName": "Alice", "email": "alice@example.com" },
        "labels": { "nodes": [{"name": "bug"}] },
        "team": { "key": "ENG", "name": "Engineering" },
        "project": { "name": "Q1 Roadmap" }
    })
}

fn minimal_issue_node(identifier: &str, title: &str) -> serde_json::Value {
    serde_json::json!({
        "id": format!("uuid-{}", identifier),
        "identifier": identifier,
        "title": title,
        "description": null,
        "url": format!("https://linear.app/team/issue/{}", identifier),
        "state": null,
        "priority": null,
        "assignee": null,
        "labels": { "nodes": [] },
        "team": null,
        "project": null
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_fetch_tasks_single_page_returns_all_issues() {
    let server = MockServer::start().await;

    let body = graphql_response(
        serde_json::json!([
            single_issue_node("ENG-1", "Fix the bug"),
            single_issue_node("ENG-2", "Add the feature"),
        ]),
        false,
        None,
    );

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(header("Authorization", "test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let source = LinearIssueSource::new_with_url(
        Some("ENG".to_string()),
        None,
        None,
        vec![],
        None,
        "test-api-key".to_string(),
        format!("{}/graphql", server.uri()),
    );

    let tasks = source.fetch_tasks().await.expect("fetch should succeed");

    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].source_id, "ENG-1");
    assert_eq!(tasks[0].title, "Fix the bug");
    assert_eq!(tasks[0].assignee, Some("Alice".to_string()));
    assert_eq!(tasks[0].labels, vec!["bug"]);
    assert_eq!(tasks[0].metadata.get("team").map(String::as_str), Some("ENG"));
    assert_eq!(tasks[0].metadata.get("team_name").map(String::as_str), Some("Engineering"));
    assert_eq!(tasks[0].metadata.get("state").map(String::as_str), Some("Todo"));
    assert_eq!(tasks[0].metadata.get("priority").map(String::as_str), Some("2"));
    assert_eq!(tasks[0].metadata.get("project").map(String::as_str), Some("Q1 Roadmap"));

    assert_eq!(tasks[1].source_id, "ENG-2");
    assert_eq!(tasks[1].title, "Add the feature");
}

#[tokio::test]
async fn test_fetch_tasks_empty_result_returns_empty_vec() {
    let server = MockServer::start().await;

    let body = graphql_response(serde_json::json!([]), false, None);

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let source = LinearIssueSource::new_with_url(
        None,
        None,
        None,
        vec![],
        None,
        "test-api-key".to_string(),
        format!("{}/graphql", server.uri()),
    );

    let tasks = source.fetch_tasks().await.expect("fetch should succeed");
    assert!(tasks.is_empty());
}

#[tokio::test]
async fn test_fetch_tasks_paginates_through_multiple_pages() {
    let server = MockServer::start().await;

    // Page 1: hasNextPage=true, endCursor="cursor-abc"
    let page1 = graphql_response(
        serde_json::json!([single_issue_node("ENG-10", "Issue ten")]),
        true,
        Some("cursor-abc"),
    );
    // Page 2: hasNextPage=false (last page)
    let page2 = graphql_response(
        serde_json::json!([single_issue_node("ENG-11", "Issue eleven")]),
        false,
        None,
    );

    // Register page-1 first: wiremock matches in registration order (FIFO)
    // and skips exhausted mocks, so page-1 handles request 1 and page-2 handles
    // request 2 once page-1's up_to_n_times(1) is consumed.
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&page1))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(wiremock::matchers::body_string_contains("cursor-abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&page2))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;

    let source = LinearIssueSource::new_with_url(
        None,
        None,
        None,
        vec![],
        None,
        "test-api-key".to_string(),
        format!("{}/graphql", server.uri()),
    );

    let tasks = source.fetch_tasks().await.expect("pagination should succeed");
    assert_eq!(tasks.len(), 2);
    let ids: Vec<&str> = tasks.iter().map(|t| t.source_id.as_str()).collect();
    assert!(ids.contains(&"ENG-10"));
    assert!(ids.contains(&"ENG-11"));
}

#[tokio::test]
async fn test_fetch_tasks_http_error_returns_err() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let source = LinearIssueSource::new_with_url(
        None,
        None,
        None,
        vec![],
        None,
        "bad-key".to_string(),
        format!("{}/graphql", server.uri()),
    );

    let result = source.fetch_tasks().await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("401") && msg.contains("Linear page fetch"), "error: {}", msg);
}

#[tokio::test]
async fn test_fetch_tasks_graphql_error_returns_err() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "errors": [
            { "message": "Field 'issues' doesn't exist on type 'Query'" }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let source = LinearIssueSource::new_with_url(
        None,
        None,
        None,
        vec![],
        None,
        "test-api-key".to_string(),
        format!("{}/graphql", server.uri()),
    );

    let result = source.fetch_tasks().await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("errors") || msg.contains("Linear page fetch"), "error: {}", msg);
}

#[tokio::test]
async fn test_fetch_tasks_issues_with_missing_optional_fields() {
    let server = MockServer::start().await;

    let body = graphql_response(
        serde_json::json!([minimal_issue_node("ENG-99", "Minimal issue")]),
        false,
        None,
    );

    Mock::given(method("POST"))
        .and(path("/graphql"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let source = LinearIssueSource::new_with_url(
        None,
        None,
        None,
        vec![],
        None,
        "test-api-key".to_string(),
        format!("{}/graphql", server.uri()),
    );

    let tasks = source.fetch_tasks().await.expect("fetch should succeed");
    assert_eq!(tasks.len(), 1);
    let task = &tasks[0];

    assert_eq!(task.source_id, "ENG-99");
    assert_eq!(task.title, "Minimal issue");
    assert_eq!(task.body, "");
    assert!(task.assignee.is_none());
    assert!(task.labels.is_empty());
    assert!(!task.metadata.contains_key("state"));
    assert!(!task.metadata.contains_key("priority"));
    assert!(!task.metadata.contains_key("team"));
    assert!(!task.metadata.contains_key("project"));
    // linear_id and identifier are always set
    assert_eq!(task.metadata.get("identifier").map(String::as_str), Some("ENG-99"));
    assert!(task.metadata.contains_key("linear_id"));
}

#[tokio::test]
async fn test_fetch_tasks_sends_authorization_header() {
    let server = MockServer::start().await;

    let body = graphql_response(serde_json::json!([]), false, None);

    // Strict matcher: only match if Authorization header is correct.
    Mock::given(method("POST"))
        .and(path("/graphql"))
        .and(header("Authorization", "lin_api_mykey123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let source = LinearIssueSource::new_with_url(
        None,
        None,
        None,
        vec![],
        None,
        "lin_api_mykey123".to_string(),
        format!("{}/graphql", server.uri()),
    );

    // Should succeed because the Authorization header matches.
    let tasks = source.fetch_tasks().await.expect("request with correct auth should succeed");
    assert!(tasks.is_empty());
}
