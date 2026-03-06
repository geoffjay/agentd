//! Monitor Service - System health monitoring and alerting daemon.
//!
//! The `agentd-monitor` service provides a REST API for querying system health
//! metrics and active alerts. Full metric collection will be added as the
//! service matures.
//!
//! # REST API Endpoints
//!
//! ## GET /health
//!
//! Health check endpoint.
//!
//! ```bash
//! curl http://localhost:17003/health
//! ```
//!
//! ## GET /metrics
//!
//! Returns the latest system metric reports.
//!
//! ```bash
//! curl http://localhost:17003/metrics
//! ```
//!
//! ## GET /alerts
//!
//! Returns currently active alerts.
//!
//! ```bash
//! curl http://localhost:17003/alerts
//! ```
//!
//! # Environment Variables
//!
//! - `PORT` - Port to listen on (default: 17003 dev, 7003 production)
//! - `RUST_LOG` - Logging level (default: info)

pub mod api;
pub mod error;
pub mod types;
