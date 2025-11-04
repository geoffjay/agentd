# agentd CLI Tests

This directory contains integration tests for the agentd CLI.

## Quick Start

```bash
# Run all tests
cargo test -p agentd-cli

# Run only integration tests
cargo test -p agentd-cli --test integration_test

# Run specific test module
cargo test -p agentd-cli notification_tests

# Run single test
cargo test -p agentd-cli test_create_notification_success
```

## Test Organization

```
integration_test.rs
├── notification_tests (13 tests)
│   ├── Create operations (4)
│   ├── List operations (4)
│   ├── Get operations (2)
│   ├── Delete operations (2)
│   └── Respond operations (2)
├── ask_service_tests (6 tests)
│   ├── Trigger operations (3)
│   └── Answer operations (3)
├── error_handling_tests (6 tests)
│   └── HTTP error scenarios
└── client_tests (6 tests)
    └── HTTP client methods
```

## Test Coverage

- **31 integration tests** covering all API endpoints
- **Mock HTTP servers** using mockito
- **No external dependencies** - tests run without services
- **Fast execution** - ~40ms for all integration tests

## Writing New Tests

Use the test helper functions:

```rust
use super::*;

#[tokio::test]
async fn test_my_endpoint() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/my-endpoint")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"key": "value"}"#)
        .create_async()
        .await;

    let client = ApiClient::new(server.url());
    let result: Result<MyType, _> = client.get("/my-endpoint").await;

    assert!(result.is_ok());
    mock.assert_async().await;
}
```

## Documentation

See:
- `../TESTING.md` - Comprehensive testing guide
- `../TEST_SUMMARY.md` - Test suite overview
