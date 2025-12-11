# BAML Rust Client

Rust client for interacting with the BAML (Basically a Made-up Language) server running via `baml serve`.

## Overview

This crate provides a type-safe, idiomatic Rust interface to all BAML functions defined in `baml_src/`. The BAML server provides AI-powered intelligence for:

- **Notifications**: Auto-categorization, digests, grouping
- **Questions**: Context-aware generation and analysis
- **Monitoring**: Log analysis, health assessment, anomaly detection
- **CLI**: Natural language parsing and help
- **Hooks**: Shell event analysis and filtering

## Quick Start

### 1. Start BAML Server

```bash
# Ensure Ollama is running (for local LLM)
ollama serve
ollama pull llama3.2

# Start BAML server from project root
cd /path/to/agentd
baml serve
```

### 2. Use the Client

```rust
use baml::{BamlClient, BamlClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let client = BamlClient::default();

    // Categorize a notification
    let result = client.categorize_notification(
        "Database Error",
        "Connection failed to PostgreSQL",
        "production monitoring"
    ).await?;

    println!("Category: {}", result.category);
    println!("Priority: {}", result.priority);

    Ok(())
}
```

## Configuration

### Default Configuration

By default, the client connects to `http://localhost:2024` with a 30-second timeout and 2 retries.

### Custom Configuration

```rust
use baml::{BamlClient, BamlClientConfig};

let config = BamlClientConfig::new("http://baml-server:8080")
    .with_timeout(60)        // 60 second timeout
    .with_max_retries(3);    // retry up to 3 times

let client = BamlClient::new(config);
```

### Environment-Based Configuration

```rust
let base_url = std::env::var("BAML_SERVER_URL")
    .unwrap_or_else(|_| "http://localhost:2024".to_string());

let config = BamlClientConfig::new(base_url);
let client = BamlClient::new(config);
```

## API Reference

### Notification Functions

```rust
// Auto-categorize notification
let category = client.categorize_notification(
    "title",
    "message",
    "source"
).await?;

// Generate digest
let digest = client.summarize_notifications(
    &["notif1", "notif2"],
    "last 24 hours"
).await?;

// Group related notifications
let groups = client.group_related_notifications(
    &notification_map,
    2  // min group size
).await?;

// Check if notification still relevant
let relevant = client.is_notification_still_relevant(
    "title",
    "message",
    24,  // hours ago
    "current state"
).await?;
```

### Question Functions

```rust
// Generate contextual question
let question = client.generate_system_question(
    "check_type",
    "system state",
    "user context",
    "recent history"
).await?;

// Analyze user's answer
let analysis = client.analyze_answer(
    "original question",
    "user answer",
    "yes_no"  // expected type
).await?;

// Generate follow-up question
let followup = client.generate_followup_question(
    "original question",
    "ambiguous answer",
    "why clarification needed"
).await?;

// Evaluate question effectiveness
let feedback = client.evaluate_question_effectiveness(
    "question text",
    15,  // response time seconds
    "user answer",
    "system outcome"
).await?;

// Personalize question for user
let personalized = client.personalize_question(
    "base question",
    "user preferences",
    "interaction history"
).await?;
```

### Monitoring Functions

```rust
// Analyze logs for errors
let analysis = client.analyze_logs(
    "service-name",
    &log_entries,
    "time window"
).await?;

// Detect patterns in logs
let patterns = client.detect_log_patterns(
    &log_entries,
    "1 hour"
).await?;

// Assess service health
let health = client.assess_service_health(
    "service-name",
    &recent_logs,
    "metrics json",
    "expected behavior"
).await?;

// Detect performance anomaly
let anomaly = client.detect_performance_anomaly(
    "metric_name",
    "current_value",
    &historical_values,
    "baseline description"
).await?;

// Correlate errors across services
let correlation = client.correlate_service_errors(
    &service_error_map,
    "time window"
).await?;
```

### CLI Functions

```rust
// Parse natural language command
let intent = client.parse_natural_language_command(
    "remind me about deployment",
    "current context"
).await?;

// Suggest command correction
let suggestion = client.suggest_command_correction(
    "notif list",
    "error message"
).await?;

// Provide natural language help
let help = client.provide_natural_language_help(
    "how do I create a notification?",
    "available commands"
).await?;

// Explain command before execution
let explanation = client.explain_command(
    "agent notify delete --all",
    "current context"
).await?;

// Suggest command aliases
let aliases = client.suggest_command_aliases(
    &command_history,
    &frequency_map
).await?;
```

### Hook Functions

```rust
// Analyze shell event
let action = client.analyze_shell_event(
    "cargo build --release",
    0,  // exit code
    "output text",
    120000,  // duration ms
    "context"
).await?;

// Learn command patterns
let patterns = client.learn_command_patterns(
    &command_history,
    &notification_history
).await?;

// Analyze command intent
let insight = client.analyze_command_intent(
    "npm run build",
    "execution context"
).await?;

// Generate completion notification
let notification = client.generate_completion_notification(
    "cargo test",
    0,  // exit code
    5000,  // duration ms
    "output summary",
    0  // previous attempts
).await?;

// Filter relevant output
let filtered = client.filter_relevant_output(
    "full command output",
    1,  // exit code
    5  // max lines
).await?;
```

## Error Handling

All methods return `Result<T, BamlError>`. Common errors:

```rust
use baml::{BamlClient, BamlError};

match client.categorize_notification("title", "msg", "src").await {
    Ok(result) => println!("Success: {:?}", result),
    Err(BamlError::ServerUnreachable { url, .. }) => {
        eprintln!("BAML server not running at {}", url);
        eprintln!("Start it with: baml serve");
    }
    Err(BamlError::Timeout { timeout_secs }) => {
        eprintln!("Request timed out after {} seconds", timeout_secs);
    }
    Err(BamlError::FunctionNotFound { function_name }) => {
        eprintln!("Function '{}' not found on server", function_name);
        eprintln!("Check BAML definitions in baml_src/");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Integration Examples

### agentd-notify Service

```rust
use baml::BamlClient;
use notify::types::CreateNotificationRequest;

async fn create_notification_with_ai_categorization(
    baml: &BamlClient,
    req: CreateNotificationRequest
) -> Result<Notification, Error> {
    // Use AI to determine priority if not specified
    let category = baml.categorize_notification(
        &req.title,
        &req.message,
        &format!("{:?}", req.source)
    ).await?;

    // Override priority with AI suggestion
    let priority = match category.priority.as_str() {
        "urgent" => NotificationPriority::Urgent,
        "high" => NotificationPriority::High,
        "normal" => NotificationPriority::Normal,
        _ => NotificationPriority::Low,
    };

    // Create notification with AI-determined priority
    create_notification_internal(req.with_priority(priority))
}
```

### agentd-ask Service

```rust
use baml::BamlClient;

async fn generate_smart_question(
    baml: &BamlClient,
    check_type: &str,
    system_state: &str
) -> Result<Question, Error> {
    // Generate contextual question using AI
    let question = baml.generate_system_question(
        check_type,
        system_state,
        "user context from history",
        "recent relevant events"
    ).await?;

    // Create question with AI-generated content
    Question {
        text: question.question_text,
        suggested_responses: question.suggested_responses,
        urgency: parse_urgency(&question.urgency),
    }
}
```

### agentd-monitor Service

```rust
use baml::BamlClient;

async fn analyze_service_logs(
    baml: &BamlClient,
    service_name: &str,
    logs: Vec<String>
) -> Result<(), Error> {
    // Analyze logs with AI
    let analysis = baml.analyze_logs(
        service_name,
        &logs,
        "last 5 minutes"
    ).await?;

    // Take action if errors detected
    if analysis.has_errors {
        eprintln!("Errors detected in {}: {}", service_name, analysis.error_summary);

        if analysis.requires_immediate_attention {
            send_alert(&analysis)?;
        }

        for action in &analysis.suggested_actions {
            eprintln!("Suggested: {}", action);
        }
    }

    Ok(())
}
```

## Examples

See the `examples/` directory for complete working examples:

```bash
# Run example (ensure BAML server is running first)
cargo run --example categorize_notification
```

## Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests (requires running BAML server)

```bash
# Terminal 1: Start BAML server
baml serve

# Terminal 2: Run tests
cargo test -- --ignored
```

## Troubleshooting

### Server Unreachable

**Error:** `BAML server unreachable at http://localhost:2024`

**Solution:**

```bash
# Check if BAML server is running
curl http://localhost:2024/health

# Start BAML server
cd /path/to/agentd
baml serve
```

### Function Not Found

**Error:** `BAML function 'XYZ' not found on server`

**Solutions:**

1. Verify function exists in `baml_src/*.baml`
2. Restart BAML server to reload functions
3. Check for typos in function name

### Timeout Errors

**Error:** `Request timed out after 30 seconds`

**Solutions:**

1. Increase timeout: `config.with_timeout(60)`
2. Use faster model: `LocalOllamaFast` in `baml_src/clients.baml`
3. Reduce input size (fewer logs, shorter text)

### Ollama Not Running

**Error:** BAML server logs show connection errors to Ollama

**Solution:**

```bash
# Start Ollama
ollama serve

# Pull model if not present
ollama pull llama3.2
```

## Performance

### Typical Latencies

- **Local Ollama** (llama3.2): 200-300ms
- **Local Ollama** (qwen2.5:3b): 100-150ms
- **Cloud Fallback** (Claude Haiku): 200-400ms

### Optimization Tips

1. Use `AgentdFast` client for low-latency operations
2. Implement caching for repeated requests
3. Batch operations where possible
4. Use smaller models for simple tasks

## See Also

- [BAML Documentation](https://docs.boundaryml.com/home)
- [BAML Source Definitions](../../baml_src/)
- [BAML Research Document](../../docs/research/baml-investigation.md)
- [Project README](../../README.md)

## License

MIT OR Apache-2.0
