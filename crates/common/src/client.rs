//! Generic HTTP client base for agentd service clients.
//!
//! This module will contain:
//! - `ServiceClient` — base struct with typed `get`, `post`, `put`, `delete`
//!   methods and consistent error handling
//! - Response parsing and error context helpers
//!
//! Individual service clients (NotifyClient, AskClient, WrapClient,
//! OrchestratorClient) will compose or extend this base.
//!
//! See #49 for migration details.
