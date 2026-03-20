//! Integration tests for the communicate service.
//!
//! Tests the full REST API flow and real-time WebSocket delivery using an
//! in-process Axum router for HTTP calls and a real TCP server for WebSocket
//! connections.
//!
//! # Design
//!
//! HTTP-only tests use `tower::ServiceExt::oneshot` directly on a cloned
//! `axum::Router`.  This avoids real TCP and is faster than spawning a server.
//!
//! WebSocket tests need a real TCP connection, so those tests call
//! `start_server()` which binds `127.0.0.1:0`, spawns `axum::serve`, and
//! returns the bound address.  HTTP calls in those tests reuse the same
//! `Router` clone (sharing the underlying `Arc<ConnectionManager>`) so
//! broadcasts triggered by HTTP requests propagate to active WS subscribers.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build a test router backed by an isolated SQLite database.
async fn build_test_app() -> (Router, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let router = communicate::build_router(&db_path).await.unwrap();
    (router, temp_dir)
}

/// Parse the response body as JSON.
async fn body_json(body: Body) -> Value {
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// POST /rooms — create a room and return its ID string.
async fn create_room(app: &Router, name: &str) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/rooms")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "name": name, "created_by": "test-agent" }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    body_json(resp.into_body()).await["id"].as_str().unwrap().to_string()
}

/// POST /rooms/{id}/participants — add a participant and assert success.
async fn add_participant(app: &Router, room_id: &str, identifier: &str) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/rooms/{room_id}/participants"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "identifier": identifier,
                        "kind": "agent",
                        "display_name": identifier,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

/// POST /rooms/{id}/messages — send a message and return the response body.
async fn send_message(app: &Router, room_id: &str, sender_id: &str, content: &str) -> Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/rooms/{room_id}/messages"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "sender_id": sender_id,
                        "sender_name": sender_id,
                        "sender_kind": "agent",
                        "content": content,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    body_json(resp.into_body()).await
}

/// Start a real TCP server on a random port and return `(addr, router, temp_dir)`.
///
/// The returned `router` shares the same `Arc<ConnectionManager>` as the
/// running server, so HTTP calls via `oneshot` on the router will trigger
/// WebSocket broadcasts to clients connected to the server.
async fn start_server() -> (String, Router, TempDir) {
    let (app, temp_dir) = build_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let app_for_server = app.clone();
    tokio::spawn(async move {
        axum::serve(listener, app_for_server).await.unwrap();
    });
    (addr, app, temp_dir)
}

// ---------------------------------------------------------------------------
// Full API flow tests (HTTP-only, no TCP server needed)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_full_api_flow_create_room_add_participant_send_list() {
    let (app, _temp) = build_test_app().await;

    // Create room.
    let room_id = create_room(&app, "integration-room").await;

    // Add a participant.
    add_participant(&app, &room_id, "agent-alice").await;

    // Send a message.
    let msg = send_message(&app, &room_id, "agent-alice", "Hello from integration test!").await;
    assert_eq!(msg["content"], "Hello from integration test!");
    assert_eq!(msg["sender_id"], "agent-alice");
    assert_eq!(msg["status"], "sent");

    // List messages.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/rooms/{room_id}/messages"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let list = body_json(resp.into_body()).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert_eq!(list["items"][0]["content"], "Hello from integration test!");
}

#[tokio::test]
async fn test_concurrent_message_sending_to_same_room() {
    let (app, _temp) = build_test_app().await;

    let room_id = create_room(&app, "concurrent-room").await;
    for i in 1..=3 {
        add_participant(&app, &room_id, &format!("agent-{i}")).await;
    }

    // Send three messages concurrently from different agents.
    let futs = (1usize..=3).map(|i| {
        let app = app.clone();
        let room_id = room_id.clone();
        async move {
            send_message(&app, &room_id, &format!("agent-{i}"), &format!("concurrent msg {i}"))
                .await
        }
    });
    let results = futures::future::join_all(futs).await;

    assert_eq!(results.len(), 3);
    for r in &results {
        assert!(r["id"].as_str().is_some(), "each message should have an id");
    }

    // Verify all three messages were persisted.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/rooms/{room_id}/messages"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list = body_json(resp.into_body()).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_message_pagination_limit_and_offset() {
    let (app, _temp) = build_test_app().await;

    let room_id = create_room(&app, "pagination-room").await;
    add_participant(&app, &room_id, "agent-page").await;

    // Persist 10 messages.
    for i in 1..=10 {
        send_message(&app, &room_id, "agent-page", &format!("message {i}")).await;
    }

    // First page: limit=5.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/rooms/{room_id}/messages?limit=5"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let page1 = body_json(resp.into_body()).await;
    assert_eq!(page1["items"].as_array().unwrap().len(), 5);
    assert_eq!(page1["total"], 10);

    // Second page: limit=5, offset=5.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/rooms/{room_id}/messages?limit=5&offset=5"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let page2 = body_json(resp.into_body()).await;
    assert_eq!(page2["items"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn test_room_deletion_cascades_to_participants_and_messages() {
    let (app, _temp) = build_test_app().await;

    let room_id = create_room(&app, "cascade-room").await;
    add_participant(&app, &room_id, "agent-cascade").await;
    send_message(&app, &room_id, "agent-cascade", "will be deleted").await;

    // Delete the room.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/rooms/{room_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Room should now return 404.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/rooms/{room_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Messages endpoint for the deleted room should also return 404 (cascade).
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/rooms/{room_id}/messages"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_endpoint_returns_ok_with_service_metadata() {
    let (app, _temp) = build_test_app().await;

    let resp = app
        .clone()
        .oneshot(Request::builder().method("GET").uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["status"], "ok");
    assert!(json["service"].as_str().is_some(), "health response should include a service name");
}

#[tokio::test]
async fn test_get_participant_by_identifier() {
    let (app, _temp) = build_test_app().await;

    let room_id = create_room(&app, "participant-get-room").await;
    add_participant(&app, &room_id, "agent-lookup").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/rooms/{room_id}/participants/agent-lookup"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    assert_eq!(json["identifier"], "agent-lookup");
}

#[tokio::test]
async fn test_list_rooms_for_participant() {
    let (app, _temp) = build_test_app().await;

    let room1 = create_room(&app, "rooms-for-p-1").await;
    let room2 = create_room(&app, "rooms-for-p-2").await;
    add_participant(&app, &room1, "multi-room-agent").await;
    add_participant(&app, &room2, "multi-room-agent").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/participants/multi-room-agent/rooms")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = body_json(resp.into_body()).await;
    assert!(
        json["items"].as_array().unwrap().len() >= 2,
        "participant should appear in both rooms"
    );
}

// ---------------------------------------------------------------------------
// WebSocket real-time delivery tests (require a real TCP server)
// ---------------------------------------------------------------------------

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Connect a WebSocket client to the given address and return the split stream.
async fn ws_connect(
    addr: &str,
    identifier: &str,
) -> (futures::stream::SplitSink<WsStream, WsMessage>, futures::stream::SplitStream<WsStream>) {
    let url =
        format!("ws://{addr}/ws?identifier={identifier}&kind=agent&display_name={identifier}");
    let (ws_stream, _) = connect_async(&url).await.unwrap();
    ws_stream.split()
}

/// Read the next text frame from a WS stream (with 500 ms timeout).
async fn ws_next_text(stream: &mut futures::stream::SplitStream<WsStream>) -> Value {
    let msg = tokio::time::timeout(std::time::Duration::from_millis(500), stream.next())
        .await
        .expect("timed out waiting for WebSocket frame")
        .expect("WebSocket stream ended unexpectedly")
        .expect("WebSocket error");

    let text = match msg {
        WsMessage::Text(t) => t.to_string(),
        other => panic!("Expected a text WebSocket frame, got: {:?}", other),
    };
    serde_json::from_str(&text).expect("WS frame was not valid JSON")
}

#[tokio::test]
async fn test_websocket_ping_pong() {
    let (addr, _app, _temp) = start_server().await;

    let (mut sink, mut stream) = ws_connect(&addr, "agent-ping").await;

    sink.send(WsMessage::Text(serde_json::to_string(&json!({ "type": "ping" })).unwrap().into()))
        .await
        .unwrap();

    let val = ws_next_text(&mut stream).await;
    assert_eq!(val["type"], "pong", "server should respond to ping with pong");

    sink.close().await.unwrap();
}

#[tokio::test]
async fn test_websocket_subscribe_and_receive_real_time_message() {
    let (addr, app, _temp) = start_server().await;

    // Set up room + two participants.
    let room_id = create_room(&app, "ws-realtime-room").await;
    add_participant(&app, &room_id, "ws-subscriber").await;
    add_participant(&app, &room_id, "ws-sender").await;

    // Connect the subscriber via WebSocket.
    let (mut sink, mut stream) = ws_connect(&addr, "ws-subscriber").await;

    // Subscribe to the room.
    sink.send(WsMessage::Text(
        serde_json::to_string(&json!({ "type": "subscribe", "room_id": room_id })).unwrap().into(),
    ))
    .await
    .unwrap();

    // Allow the server to register the subscription before sending.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Post a message via HTTP (shared router state → same ConnectionManager).
    send_message(&app, &room_id, "ws-sender", "real-time delivery!").await;

    // The WS subscriber should receive the broadcast.
    let val = ws_next_text(&mut stream).await;
    assert_eq!(val["type"], "message", "subscriber should receive a 'message' event");
    assert_eq!(
        val["message"]["content"], "real-time delivery!",
        "message content should match what was sent"
    );

    sink.close().await.unwrap();
}

#[tokio::test]
async fn test_websocket_subscribe_non_participant_receives_error() {
    let (addr, app, _temp) = start_server().await;

    let room_id = create_room(&app, "restricted-ws-room").await;
    // Do NOT add the WS client as a participant.

    let (mut sink, mut stream) = ws_connect(&addr, "ws-outsider").await;

    sink.send(WsMessage::Text(
        serde_json::to_string(&json!({ "type": "subscribe", "room_id": room_id })).unwrap().into(),
    ))
    .await
    .unwrap();

    let val = ws_next_text(&mut stream).await;
    assert_eq!(
        val["type"], "error",
        "server should return an error when the client is not a room participant"
    );

    sink.close().await.unwrap();
}

#[tokio::test]
async fn test_websocket_unsubscribe_stops_delivery() {
    let (addr, app, _temp) = start_server().await;

    let room_id = create_room(&app, "unsub-room").await;
    add_participant(&app, &room_id, "ws-unsub-agent").await;
    add_participant(&app, &room_id, "other-sender").await;

    let (mut sink, mut stream) = ws_connect(&addr, "ws-unsub-agent").await;

    // Subscribe.
    sink.send(WsMessage::Text(
        serde_json::to_string(&json!({ "type": "subscribe", "room_id": room_id })).unwrap().into(),
    ))
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Unsubscribe.
    sink.send(WsMessage::Text(
        serde_json::to_string(&json!({ "type": "unsubscribe", "room_id": room_id }))
            .unwrap()
            .into(),
    ))
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Post a message — the unsubscribed client should NOT receive it.
    send_message(&app, &room_id, "other-sender", "you shouldn't see this").await;

    let result = tokio::time::timeout(std::time::Duration::from_millis(200), stream.next()).await;

    // A timeout here is exactly what we want — no message delivered after unsubscribe.
    assert!(result.is_err(), "unsubscribed client should not receive further messages");

    sink.close().await.unwrap();
}
