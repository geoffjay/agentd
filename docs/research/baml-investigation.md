# BAML Investigation and Integration Plan

**Date:** 2025-11-08
**Author:** Research conducted by Claude
**Status:** Implementation in progress

## Executive Summary

This document presents the findings from investigating BAML (Basically a Made-up Language) as a framework for improving prompt creation and LLM integration in the agentd project. The investigation concludes that BAML is an excellent fit for agentd and provides a comprehensive integration plan.

## What is BAML?

**BAML** is a domain-specific language (DSL) for building reliable AI workflows and agents. It transforms prompt engineering into "schema engineering" by treating prompts as typed functions with structured inputs and outputs.

### Core Philosophy

Rather than writing prompts as unstructured strings scattered throughout code, BAML:
- Defines prompts as first-class functions with typed inputs and outputs
- Enforces compile-time type safety for LLM interactions
- Separates prompt definitions from application logic
- Generates native client libraries for multiple programming languages

### Technical Architecture

- **Language**: Domain-specific language for defining LLM functions, data models, and client configurations
- **Compiler**: Built in Rust for performance
- **Output**: Generates native client libraries (Python, TypeScript, Ruby, Go, Java, C#, **Rust**)
- **Storage**: `.baml` files stored locally, no cloud dependency
- **Version Control**: Integrates seamlessly with Git

## Key Features

### 1. Type-Safe LLM Functions

Prompts are defined as functions with typed parameters and return types:

```baml
class Resume {
  name string
  email string
  experience string[]
  skills string[]
}

function ExtractResume(resume: string) -> Resume {
  client "openai/gpt-5-mini"
  prompt #"
    Extract structured data from:
    {{ resume }}

    {{ ctx.output_format }}
  "#
}
```

### 2. Multi-Language Support

BAML generates clients for:
- **Rust** ✓ (our primary language)
- Python (Pydantic models)
- TypeScript
- Ruby (Sorbet types)
- Go
- Java
- C#

### 3. Model Flexibility

Supports 100+ LLM providers with single-line swapping:
- OpenAI (GPT-4, GPT-5)
- Anthropic Claude (Opus, Sonnet, Haiku)
- Google Gemini
- AWS Bedrock
- Azure OpenAI
- Vertex AI
- **Ollama** (local models) ✓

### 4. Schema-Aligned Parsing (SAP)

Proprietary algorithm enables structured outputs even with models lacking native function-calling APIs. Handles flexible LLM response formats and extracts structured data reliably.

### 5. Declarative Client Configuration

```baml
client<llm> CustomOllama {
  provider openai-generic
  options {
    base_url "http://localhost:11434/v1"
    model "llama4"
    default_role "user"
  }
}

// Fallback strategy
client<llm> ResilientClient {
  provider fallback
  options {
    strategy [LocalOllama, CloudClaude, OpenAIBackup]
  }
}

// Round-robin load balancing
client<llm> LoadBalanced {
  provider round-robin
  options {
    strategy [Model1, Model2, Model3]
  }
}

// Retry policies
retry_policy Exponential {
  max_retries 3
  strategy {
    type exponential_backoff
    delay_ms 300
    multiplier 1.5
    max_delay_ms 10000
  }
}
```

### 6. Streaming Support

Fully type-safe streaming with partial response handling for real-time UI updates.

### 7. IDE Tooling

VSCode extension provides:
- Prompt visualization
- API request inspection
- Rapid iteration (testing cycles from minutes to seconds)
- Syntax highlighting and autocomplete

## Evaluation for agentd

### Alignment with Project Goals

| Criterion | Assessment | Notes |
|-----------|------------|-------|
| **Rust Compatibility** | ✓ Excellent | Generates native Rust clients |
| **Ollama Integration** | ✓ Excellent | Out-of-box support, no API keys needed |
| **Service Architecture** | ✓ Excellent | Fits REST API microservices pattern |
| **Type Safety** | ✓ Excellent | Compile-time checks for LLM interactions |
| **Local-First** | ✓ Excellent | No cloud dependency, works with local Ollama |
| **Learning Curve** | ⚠ Moderate | New DSL to learn, but good documentation |
| **Project Maturity** | ✓ Good | Used in production, active development |

### Why BAML Fits agentd

1. **Rust-First Project**: agentd is written entirely in Rust; BAML generates Rust clients
2. **Planned Ollama Integration**: README mentions "AI integration via Ollama" - BAML supports this natively
3. **Structured Interactions**: Ask service already handles structured questions/answers - BAML formalizes this
4. **Empty Ollama Crate**: `crates/ollama/src/lib.rs` is currently a TODO - perfect foundation
5. **Microservices Pattern**: BAML functions integrate cleanly with existing REST APIs
6. **Type Safety Culture**: Project uses strong typing throughout - BAML extends this to LLM calls

### Trade-offs

**Advantages:**
- Type-safe LLM interactions
- Easy model swapping and fallback strategies
- Testable prompt engineering
- Clear separation of concerns
- Version-controlled prompt definitions
- Local execution (no external dependencies with Ollama)

**Considerations:**
- New toolchain and DSL to learn
- Additional build step (BAML compilation)
- External dependency (though well-maintained)
- May be overkill for very simple prompt use cases

**Verdict: Benefits significantly outweigh costs for agentd's use cases.**

## Practical Use Cases for agentd

### 1. Smart Notification Categorization

**Current State**: Manual priority and lifetime assignment
**With BAML**: AI-powered categorization based on content analysis

```baml
class NotificationCategory {
  category string  // "urgent", "info", "action_required", "reminder"
  priority string  // "low", "normal", "high", "urgent"
  suggested_lifetime string  // "ephemeral", "persistent"
  reasoning string
}

function CategorizeNotification(
  title: string,
  message: string,
  source_context: string
) -> NotificationCategory {
  client "ollama/llama4"
  prompt #"
    Analyze this notification and determine its category and priority:

    Title: {{ title }}
    Message: {{ message }}
    Source: {{ source_context }}

    Consider:
    - Urgency indicators (deadlines, system failures)
    - Action requirements (user decisions needed)
    - Information value (FYI vs actionable)

    {{ ctx.output_format }}
  "#
}
```

**Integration Point**: `agentd-notify` service
**Benefit**: Automatic smart prioritization reduces notification noise

### 2. Intelligent Question Generation

**Current State**: Hardcoded question templates (e.g., tmux check)
**With BAML**: Context-aware question generation

```baml
class SystemQuestion {
  question_text string
  suggested_responses string[]
  reasoning string
  urgency string
  follow_up_actions string[]
}

function GenerateSystemQuestion(
  check_type: string,
  system_state: string,
  user_context: string,
  recent_history: string
) -> SystemQuestion {
  client "ollama/llama4"
  prompt #"
    Based on the system state, generate an appropriate question for the user:

    Check Type: {{ check_type }}
    System State: {{ system_state }}
    User Context: {{ user_context }}
    Recent History: {{ recent_history }}

    Generate a clear, actionable question with suggested responses.
    The question should be:
    - Specific to the current situation
    - Actionable (user can make a clear decision)
    - Contextual (references relevant information)

    {{ ctx.output_format }}
  "#
}
```

**Integration Point**: `agentd-ask` service
**Benefit**: Dynamic questions adapt to system state and user patterns

### 3. Notification Summary and Digest

**Current State**: Manual review of all notifications
**With BAML**: Intelligent daily/weekly digests

```baml
class NotificationDigest {
  summary string
  key_actions string[]
  urgent_count int
  categories map<string, int>
  trends string
  recommendations string[]
}

function SummarizeNotifications(
  notifications: string[],
  time_period: string
) -> NotificationDigest {
  client "anthropic/claude-sonnet-4"
  prompt #"
    Summarize these notifications from {{ time_period }}:

    {{ notifications }}

    Provide:
    - High-level summary of what happened
    - Key actions required
    - Trends or patterns
    - Recommendations for the user

    {{ ctx.output_format }}
  "#
}
```

**Integration Point**: GUI application, scheduled task
**Benefit**: Users can quickly understand notification activity without reading everything

### 4. Log Analysis and Error Detection

**Current State**: Manual log review
**With BAML**: Automated error detection and diagnosis

```baml
class LogAnalysis {
  has_errors bool
  error_summary string
  affected_services string[]
  suggested_actions string[]
  severity string  // "info", "warning", "error", "critical"
  root_cause string
}

function AnalyzeLogs(
  service_name: string,
  log_entries: string[],
  time_window: string
) -> LogAnalysis {
  client "ollama/llama4"
  prompt #"
    Analyze these logs from {{ service_name }} ({{ time_window }}):

    {{ log_entries }}

    Identify:
    - Errors and their severity
    - Root cause analysis
    - Affected services/components
    - Recommended fixes

    {{ ctx.output_format }}
  "#
}
```

**Integration Point**: `agentd-monitor` service
**Benefit**: Proactive error detection with actionable recommendations

### 5. Natural Language CLI Interface

**Current State**: Structured command syntax
**With BAML**: Natural language command parsing

```baml
class CommandIntent {
  action string  // "create_notification", "list_notifications", "query_status"
  parameters map<string, string>
  confidence float
  requires_confirmation bool
}

function ParseUserCommand(
  user_input: string,
  context: string
) -> CommandIntent {
  client "ollama/llama4"
  prompt #"
    Parse this natural language command:

    "{{ user_input }}"

    Context: {{ context }}

    Determine:
    - The intended action from available commands
    - Required parameters and their values
    - Confidence level (0.0 - 1.0)
    - Whether confirmation is needed

    Available actions: create_notification, list_notifications, get_notification,
    delete_notification, respond_to_notification, trigger_ask_check, answer_question

    {{ ctx.output_format }}
  "#
}
```

**Integration Point**: `agent` CLI
**Benefit**: Users can interact naturally: `agent "remind me about the deployment tomorrow at 2pm"`

### 6. Hook Event Intelligence

**Current State**: Basic hook triggers (planned)
**With BAML**: Intelligent hook analysis

```baml
class HookAction {
  should_notify bool
  notification_title string
  notification_message string
  priority string
  reasoning string
  metadata map<string, string>
}

function AnalyzeShellEvent(
  command: string,
  exit_code: int,
  output: string,
  duration_ms: int,
  context: string
) -> HookAction {
  client "ollama/llama4"
  prompt #"
    Analyze this shell command execution:

    Command: {{ command }}
    Exit Code: {{ exit_code }}
    Duration: {{ duration_ms }}ms
    Output: {{ output }}
    Context: {{ context }}

    Determine:
    - Whether this warrants a notification
    - Appropriate priority level
    - Clear, actionable notification message
    - Relevant metadata for user context

    Only recommend notifications for:
    - Long-running commands completing
    - Commands with errors
    - Commands matching user-defined patterns

    {{ ctx.output_format }}
  "#
}
```

**Integration Point**: `agentd-hook` service
**Benefit**: Smart filtering reduces notification fatigue

## Implementation Plan

### Project Structure

```
agentd/
├── baml_src/                    # BAML definitions
│   ├── clients.baml             # LLM client configurations
│   ├── generators.baml          # Code generation config
│   ├── notifications.baml       # Notification functions
│   ├── questions.baml           # Ask service functions
│   ├── monitoring.baml          # Log analysis functions
│   ├── cli.baml                 # CLI parsing functions
│   └── hooks.baml               # Hook analysis functions
├── crates/
│   └── ollama/
│       ├── src/
│       │   ├── lib.rs           # Public API
│       │   └── baml_client/     # Generated by BAML (gitignored)
│       ├── Cargo.toml
│       └── build.rs             # Generate BAML client
└── docs/
    └── research/
        └── baml-investigation.md  # This document
```

### Phase 1: Foundation (Week 1)

**Goal**: Set up BAML infrastructure and prove the concept

1. **Initialize BAML Project**
   ```bash
   cd agentd
   baml init --client-type rust
   ```

2. **Configure Clients**
   - Set up Ollama client (localhost:11434)
   - Configure fallback to Anthropic Claude (optional)
   - Define retry policies

3. **Implement First Use Case**
   - Start with notification categorization (simplest)
   - Create `baml_src/notifications.baml`
   - Generate Rust client
   - Add integration to `agentd-notify`

4. **Testing**
   - Unit tests for BAML functions
   - Integration tests with mock notifications
   - Local Ollama testing

**Success Criteria**: Successfully categorize notifications using local Ollama

### Phase 2: Core Integration (Week 2-3)

**Goal**: Integrate BAML into existing services

1. **Ask Service Enhancement**
   - Implement `GenerateSystemQuestion` function
   - Replace hardcoded tmux questions
   - Add context awareness

2. **Log Analysis**
   - Implement `AnalyzeLogs` function
   - Integrate with `agentd-monitor` service
   - Set up automated error detection

3. **Update Ollama Crate**
   - Create public API wrapping BAML client
   - Add convenience methods
   - Document usage patterns

**Success Criteria**: Ask service generates contextual questions; monitor detects errors

### Phase 3: Advanced Features (Week 4-5)

**Goal**: Add sophisticated AI-powered features

1. **Notification Digests**
   - Implement `SummarizeNotifications`
   - Add scheduled digest generation
   - Display in GUI

2. **Natural Language CLI**
   - Implement `ParseUserCommand`
   - Add NL mode to CLI
   - Fallback to traditional commands

3. **Hook Intelligence**
   - Implement `AnalyzeShellEvent`
   - Integrate with `agentd-hook` service
   - Smart filtering logic

**Success Criteria**: Users can interact naturally with CLI; hooks intelligently filter events

### Phase 4: Optimization (Week 6)

**Goal**: Performance tuning and production readiness

1. **Performance**
   - Benchmark LLM call latencies
   - Optimize prompt sizes
   - Implement caching where appropriate

2. **Fallback Strategies**
   - Configure multi-model fallbacks
   - Test degraded scenarios
   - Document failure modes

3. **Documentation**
   - User guide for BAML features
   - Developer guide for adding new functions
   - Troubleshooting guide

**Success Criteria**: Production-ready BAML integration with monitoring

## Technical Details

### BAML CLI Commands

```bash
# Initialize project
baml init --client-type rust

# Check for errors
baml check

# Generate client code
baml generate

# Run tests
baml test

# Start development server (with hot reload)
baml dev

# Start REPL for testing
baml repl
```

### Integration with Cargo Build

**Option 1: build.rs** (Recommended)
```rust
// crates/ollama/build.rs
use std::process::Command;

fn main() {
    // Generate BAML client during build
    let status = Command::new("baml")
        .args(&["generate"])
        .current_dir("../..")
        .status()
        .expect("Failed to run baml generate");

    if !status.success() {
        panic!("BAML generation failed");
    }

    // Rerun if BAML files change
    println!("cargo:rerun-if-changed=../../baml_src");
}
```

**Option 2: xtask**
```rust
// Add to xtask/src/main.rs
fn generate_baml() -> Result<()> {
    cmd!("baml", "generate").run()?;
    Ok(())
}
```

### Dependencies

Add to `crates/ollama/Cargo.toml`:
```toml
[dependencies]
# BAML runtime (version will be determined by baml generate)
baml-runtime = "0.213.0"
baml-types = "0.213.0"

# Required for async LLM calls
tokio = { version = "1.35", features = ["full"] }

# Existing dependencies
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
```

### Generated Code Structure

BAML generates:
```
crates/ollama/src/baml_client/
├── mod.rs                    # Client entry point
├── types.rs                  # Generated type definitions
├── functions/
│   ├── categorize_notification.rs
│   ├── generate_system_question.rs
│   └── ...
└── tracing.rs               # Observability
```

Usage in Rust:
```rust
use crate::baml_client::{BamlClient, types::*};

pub async fn categorize_notification(
    title: &str,
    message: &str,
    source: &str,
) -> Result<NotificationCategory> {
    let client = BamlClient::new();

    client.categorize_notification(
        title.to_string(),
        message.to_string(),
        source.to_string(),
    ).await
}
```

## Testing Strategy

### 1. BAML Function Tests

BAML supports built-in tests:
```baml
test categorize_urgent_notification {
  functions [CategorizeNotification]
  args {
    title "System Failure"
    message "Database connection lost"
    source_context "production database"
  }
}
```

Run with: `baml test`

### 2. Rust Integration Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_categorization() {
        let result = categorize_notification(
            "Test",
            "This is a test notification",
            "test"
        ).await.unwrap();

        assert!(result.priority == "low" || result.priority == "normal");
    }
}
```

### 3. Mock LLM for CI/CD

For testing without Ollama dependency:
- Use deterministic mock responses
- Test error handling paths
- Validate type correctness

## Monitoring and Observability

BAML provides built-in observability:

```rust
use baml_client::tracing::BamlTracing;

// Enable tracing
BamlTracing::configure()
    .with_log_level("info")
    .enable();

// Traces include:
// - Function calls and duration
// - Token usage
// - Model responses
// - Errors and retries
```

Integration with agentd logging:
```rust
use tracing::info;

info!(
    target: "ollama::baml",
    function = "CategorizeNotification",
    duration_ms = %duration,
    tokens = %tokens,
    "BAML function completed"
);
```

## Cost Considerations

### Local Ollama (Primary)
- **Cost**: Free
- **Latency**: ~100-500ms depending on model
- **Privacy**: Complete (all local)
- **Availability**: Requires Ollama running

### Cloud Fallback (Optional)
- **Anthropic Claude Haiku**: ~$0.25 per 1M input tokens
- **Estimated usage**: <1000 calls/day = <$1/month
- **Use case**: Fallback when Ollama unavailable

**Recommendation**: Use Ollama as primary, cloud as fallback

## Security Considerations

1. **API Keys**: Store in environment variables, never commit
2. **Prompt Injection**: BAML's type system provides some protection
3. **Data Privacy**: Local Ollama keeps all data on-device
4. **Rate Limiting**: Configure retry policies to prevent abuse
5. **Input Validation**: Sanitize user inputs before LLM calls

## Migration Path

### Incremental Adoption

1. **No Breaking Changes**: BAML integrates alongside existing code
2. **Feature Flags**: Gate BAML features behind flags
3. **Gradual Rollout**: Enable per-service or per-feature
4. **Fallback Logic**: Maintain non-AI code paths

Example:
```rust
pub async fn create_notification(req: CreateNotificationRequest) -> Result<Notification> {
    let priority = if cfg!(feature = "baml") {
        // AI-powered categorization
        categorize_notification(&req.title, &req.message, &req.source)
            .await
            .map(|cat| cat.priority)
            .unwrap_or(req.priority)
    } else {
        // Original behavior
        req.priority
    };

    // Rest of notification creation...
}
```

## Success Metrics

### Technical Metrics
- BAML function success rate: >95%
- Average latency: <500ms (local Ollama)
- Token usage: Track for optimization
- Error rate: <5%

### User Metrics
- Notification categorization accuracy: User feedback
- Question relevance: Answer rate improvement
- CLI NL parsing success: Command success rate
- Digest usefulness: User engagement

### Business Metrics
- Reduced notification noise
- Improved user productivity
- Faster issue detection
- Enhanced user experience

## Resources

### Documentation
- BAML Docs: https://docs.boundaryml.com/home
- GitHub: https://github.com/BoundaryML/baml
- VSCode Extension: Search "BAML" in extensions

### Community
- GitHub Issues: Report bugs and feature requests
- Community examples: Check BAML examples repo

### Internal
- This document: `docs/research/baml-investigation.md`
- BAML definitions: `baml_src/`
- Implementation: `crates/ollama/`

## Conclusion

BAML represents an excellent fit for the agentd project, providing:
- Type-safe LLM integration in Rust
- Local-first operation with Ollama
- Structured prompt engineering
- Production-ready reliability features

The proposed implementation plan provides a clear path from proof-of-concept to production deployment, with incremental adoption minimizing risk.

**Recommendation: Proceed with BAML integration following the phased approach outlined above.**

---

**Next Steps:**
1. Initialize BAML project structure
2. Configure Ollama client
3. Implement notification categorization (POC)
4. Evaluate results and iterate
