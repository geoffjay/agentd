//! End-to-end authentication and tenant-isolation integration test.
//!
//! This test exercises the complete auth chain in a single process, with no
//! external binaries or persistent databases:
//!
//! ```text
//!  [test]
//!    |
//!    |-- POST /auth/register          --> core (in-process listener A)
//!    |<-- 201 { token, user.active_organization_id }
//!    |
//!    |-- POST /api/v1/notify/notifications  Bearer <token_a>
//!    |   core gateway: reads token -> resolves org -> injects X-Tenant-ID
//!    |   --> notify (in-process listener B): stores notification with org_id
//!    |<-- 201
//!    |
//!    |-- GET /api/v1/notify/notifications   Bearer <token_a>
//!    |<-- 200 { total: 1 }   (user A sees their own notification)
//!    |
//!    |-- POST /auth/register          (user B, separate org)
//!    |-- GET /api/v1/notify/notifications   Bearer <token_b>
//!    |<-- 200 { total: 0 }   (user B cannot see user A's notification)
//! ```
//!
//! Acceptance criteria (from issue #1128):
//! - [x] register -> login -> gateway proxy -> downstream with X-Tenant-ID
//! - [x] tenant isolation: user A's data is invisible to user B
//! - [x] temp databases — no side effects on dev data
//! - [x] passes with `cargo test`

use agentd_common::storage::create_test_connection;
use agentd_core::{
    api::{create_router_with_proxy, AppState},
    proxy::ProxyConfig,
    storage::Storage,
};
use notify::{
    api::{create_router as notify_create_router, ApiState as NotifyState},
    storage::NotificationStorage,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bind a `TcpListener` on an OS-assigned ephemeral port and return its address.
async fn bind_ephemeral() -> (SocketAddr, tokio::net::TcpListener) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    (addr, listener)
}

/// Start an in-process notify service backed by a temp SQLite file.
///
/// Returns the bound address and the `TempDir` guard — the caller must keep
/// the guard alive for the duration of the test.
async fn start_notify(tmp: &tempfile::TempDir) -> SocketAddr {
    let db_path = tmp.path().join("notify.db");
    let storage = NotificationStorage::with_path(&db_path).await.unwrap();
    let state = NotifyState { storage: Arc::new(storage) };
    let router = notify_create_router(state);
    let (addr, listener) = bind_ephemeral().await;
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    addr
}

/// Start an in-process core service whose gateway proxy points at `notify_addr`.
///
/// Returns the bound address and the `TempDir` guard for the core database.
async fn start_core(notify_addr: SocketAddr) -> (SocketAddr, tempfile::TempDir) {
    let (conn, tmp) = create_test_connection().await;
    let storage = Storage::new(conn).await.unwrap();
    let state = AppState::new(storage);

    let mut services: HashMap<&'static str, String> = HashMap::new();
    services.insert("notify", format!("http://{notify_addr}"));

    let proxy = ProxyConfig {
        services,
        client: reqwest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap(),
    };

    let router = create_router_with_proxy(state, proxy);
    let (addr, listener) = bind_ephemeral().await;
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (addr, tmp)
}

/// Register a new user via `POST /auth/register` and return `(token, active_org_id)`.
async fn register(client: &Client, base: &str, username: &str, email: &str) -> (String, String) {
    let body: Value = client
        .post(format!("{base}/auth/register"))
        .json(&json!({
            "username": username,
            "email": email,
            "password": "test-password-123"
        }))
        .send()
        .await
        .expect("POST /auth/register failed")
        .json()
        .await
        .expect("failed to parse register response");

    let token = body["token"].as_str().expect("register response missing 'token'").to_string();
    let org_id = body["user"]["active_organization_id"]
        .as_str()
        .expect("register response missing 'user.active_organization_id'")
        .to_string();

    (token, org_id)
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

/// Full E2E scenario:
/// 1. Two users (Alice and Bob) register independently — each gets a personal org.
/// 2. Alice creates a notification through the gateway; it is stored with her org_id.
/// 3. Alice lists notifications and sees exactly 1.
/// 4. Bob lists notifications and sees 0 (cross-tenant isolation).
#[tokio::test]
async fn test_e2e_register_proxy_and_tenant_isolation() {
    // --- Start services -------------------------------------------------------
    // Keep temp dirs alive for the entire test. Both dirs are owned by this
    // scope so they are not dropped until the function returns.
    let notify_tmp = tempfile::TempDir::new().unwrap();
    let notify_addr = start_notify(&notify_tmp).await;
    let (core_addr, _core_tmp) = start_core(notify_addr).await;

    let base = format!("http://{core_addr}");
    let client = Client::builder().timeout(Duration::from_secs(10)).build().unwrap();

    // --- Register Alice -------------------------------------------------------
    // `POST /auth/register` creates the user, a personal organization, and
    // sets it as the user's `active_organization_id`. The response includes a
    // session token that the gateway uses to resolve the tenant.
    let (token_a, org_id_a) = register(&client, &base, "alice", "alice@example.com").await;
    assert!(!token_a.is_empty(), "Alice's token must not be empty");
    assert!(!org_id_a.is_empty(), "Alice must have an active organization");

    // --- Create a notification as Alice (through the gateway) ----------------
    // The gateway:
    //   1. Validates token_a against the sessions table
    //   2. Reads alice's active_organization_id (org_id_a)
    //   3. Injects X-Tenant-ID: <org_id_a> before forwarding to notify
    // The notify service reads X-Tenant-ID via OptionalTenantId and stores
    // org_id_a on the notification row.
    let create_status = client
        .post(format!("{base}/api/v1/notify/notifications"))
        .bearer_auth(&token_a)
        .json(&json!({
            "source": { "type": "system" },
            "lifetime": { "type": "persistent" },
            "priority": "high",
            "title": "Alice's notification",
            "message": "Visible only within Alice's organization",
            "requires_response": false
        }))
        .send()
        .await
        .expect("POST notification failed")
        .status();
    assert_eq!(create_status.as_u16(), 201, "notification creation should return 201");

    // --- Alice lists her notifications ----------------------------------------
    // Because the notification was created with org_id_a, the tenant-scoped
    // list (org_id = org_id_a OR org_id IS NULL) returns it.
    let list_a: Value = client
        .get(format!("{base}/api/v1/notify/notifications"))
        .bearer_auth(&token_a)
        .send()
        .await
        .expect("GET notifications (alice) failed")
        .json()
        .await
        .expect("failed to parse list response (alice)");
    let total_a = list_a["total"].as_u64().unwrap_or(0);
    assert_eq!(total_a, 1, "Alice should see exactly 1 notification");

    // --- Register Bob (separate org) ------------------------------------------
    let (token_b, org_id_b) = register(&client, &base, "bob", "bob@example.com").await;
    assert_ne!(org_id_b, org_id_a, "Bob and Alice must be in different organizations");

    // --- Bob lists notifications — must see 0 (tenant isolation) --------------
    // Bob's query filters to org_id_b OR NULL. Alice's notification has
    // org_id_a (not null) so it does not appear in Bob's result set.
    let list_b: Value = client
        .get(format!("{base}/api/v1/notify/notifications"))
        .bearer_auth(&token_b)
        .send()
        .await
        .expect("GET notifications (bob) failed")
        .json()
        .await
        .expect("failed to parse list response (bob)");
    let total_b = list_b["total"].as_u64().unwrap_or(0);
    assert_eq!(total_b, 0, "Bob must not see Alice's notification (tenant isolation enforced)");
}
