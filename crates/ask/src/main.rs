use anyhow::Result;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting agentd-ask daemon...");

    // Set up graceful shutdown signal handler
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        warn!("Shutdown signal received, stopping daemon...");
    };

    // Main daemon loop
    tokio::select! {
        _ = run_daemon() => {
            info!("Daemon task completed");
        }
        _ = shutdown_signal => {
            info!("Graceful shutdown initiated");
        }
    }

    info!("agentd-ask daemon stopped");
    Ok(())
}

async fn run_daemon() {
    // Main daemon logic goes here
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        // TODO: Implement daemon functionality
    }
}

/// Process a message and return a response
fn process_message(msg: &str) -> Result<String> {
    if msg.is_empty() {
        anyhow::bail!("Message cannot be empty");
    }
    Ok(format!("Processed: {}", msg))
}

/// Validate if a request is allowed
fn validate_request(request: &str) -> bool {
    !request.is_empty() && request.len() <= 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_message_success() {
        let result = process_message("hello");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Processed: hello");
    }

    #[test]
    fn test_process_message_empty() {
        let result = process_message("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Message cannot be empty");
    }

    #[test]
    fn test_validate_request_valid() {
        assert!(validate_request("valid request"));
    }

    #[test]
    fn test_validate_request_empty() {
        assert!(!validate_request(""));
    }

    #[test]
    fn test_validate_request_too_long() {
        let long_request = "x".repeat(1025);
        assert!(!validate_request(&long_request));
    }

    #[tokio::test]
    async fn test_async_operation() {
        // Example async test
        let duration = tokio::time::Duration::from_millis(10);
        tokio::time::sleep(duration).await;
        assert!(true);
    }
}
