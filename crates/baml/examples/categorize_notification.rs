//! Example: Categorize a notification using BAML
//!
//! This example shows how to use the BAML client to automatically categorize
//! a notification based on its content.
//!
//! # Prerequisites
//!
//! 1. Start Ollama (if using local LLM):
//!    ```bash
//!    ollama serve
//!    ollama pull llama3.2
//!    ```
//!
//! 2. Start BAML server:
//!    ```bash
//!    cd /path/to/agentd
//!    baml serve
//!    ```
//!
//! # Running
//!
//! ```bash
//! cargo run --example categorize_notification
//! ```

use baml::{BamlClient, BamlClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    // Create BAML client (connects to localhost:2024 by default)
    let config = BamlClientConfig::default();
    let client = BamlClient::new(config);

    println!("BAML Notification Categorization Example");
    println!("=========================================\n");

    // Example 1: Critical system error
    println!("Example 1: Critical system error");
    let result = client
        .categorize_notification(
            "Database Connection Lost",
            "Unable to connect to PostgreSQL database at db.example.com:5432. \
             Connection timeout after 30 seconds. All services are affected.",
            "production monitoring system",
        )
        .await?;

    println!("  Title: Database Connection Lost");
    println!("  Category: {}", result.category);
    println!("  Priority: {}", result.priority);
    println!("  Lifetime: {}", result.suggested_lifetime);
    println!("  Reasoning: {}", result.reasoning);
    if let Some(action) = &result.suggested_action {
        println!("  Suggested Action: {}", action);
    }
    println!();

    // Example 2: Normal informational notification
    println!("Example 2: Normal informational notification");
    let result = client
        .categorize_notification(
            "Backup Completed Successfully",
            "Daily backup finished at 2:00 AM. 2.3GB backed up to S3. \
             All files verified successfully.",
            "backup service",
        )
        .await?;

    println!("  Title: Backup Completed Successfully");
    println!("  Category: {}", result.category);
    println!("  Priority: {}", result.priority);
    println!("  Lifetime: {}", result.suggested_lifetime);
    println!("  Reasoning: {}", result.reasoning);
    println!();

    // Example 3: Action required
    println!("Example 3: Action required");
    let result = client
        .categorize_notification(
            "Approval Needed for Production Deployment",
            "The deployment pipeline has prepared version 2.1.0 for production. \
             Please review and approve before 5 PM today.",
            "CI/CD pipeline",
        )
        .await?;

    println!("  Title: Approval Needed for Production Deployment");
    println!("  Category: {}", result.category);
    println!("  Priority: {}", result.priority);
    println!("  Lifetime: {}", result.suggested_lifetime);
    println!("  Reasoning: {}", result.reasoning);
    if let Some(action) = &result.suggested_action {
        println!("  Suggested Action: {}", action);
    }
    println!();

    // Example 4: Warning about approaching limit
    println!("Example 4: Warning about approaching limit");
    let result = client
        .categorize_notification(
            "Certificate Expiring Soon",
            "SSL certificate for api.example.com will expire in 7 days. \
             Renewal is recommended to avoid service disruption.",
            "security monitoring",
        )
        .await?;

    println!("  Title: Certificate Expiring Soon");
    println!("  Category: {}", result.category);
    println!("  Priority: {}", result.priority);
    println!("  Lifetime: {}", result.suggested_lifetime);
    println!("  Reasoning: {}", result.reasoning);
    println!();

    println!("✓ All examples completed successfully!");

    Ok(())
}
