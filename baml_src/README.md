# BAML Integration for agentd

This directory contains BAML (Basically a Made-up Language) definitions for AI-powered features in agentd.

## Overview

BAML provides type-safe LLM function definitions that enable intelligent automation throughout agentd:

- **Notification Intelligence**: Auto-categorization, digests, and relevance filtering
- **Smart Questions**: Context-aware question generation for the ask service
- **Log Analysis**: Automated error detection and root cause analysis
- **CLI Intelligence**: Natural language command parsing and help
- **Hook Analysis**: Smart filtering of shell events for notifications

## Project Structure

```
baml_src/
├── README.md              # This file
├── clients.baml           # LLM client configurations (Ollama, Claude, etc.)
├── generators.baml        # Code generation configuration
├── notifications.baml     # Notification processing functions
├── questions.baml         # Ask service intelligence
├── monitoring.baml        # Log analysis and health monitoring
├── cli.baml              # Natural language CLI processing
└── hooks.baml            # Shell hook event analysis
```

## Quick Start

### Prerequisites

1. **BAML CLI**: Already installed if you can run `baml --version`
2. **Ollama** (recommended): Local LLM for privacy and cost
   ```bash
   # Install Ollama: https://ollama.ai
   # Pull a model:
   ollama pull llama3.2
   ```
3. **Optional**: Anthropic API key for fallback
   ```bash
   export ANTHROPIC_API_KEY=your_key_here
   ```

### Development Workflow

```bash
# Check for errors in BAML files
baml check

# Generate client code (Python and OpenAPI)
baml generate

# Run BAML tests
baml test

# Start development server with hot reload
baml dev

# Interactive REPL for testing functions
baml repl
```

### Testing Functions

BAML includes built-in tests for each function. Run them with:

```bash
# Run all tests
baml test

# Run specific test
baml test categorize_urgent_system_failure

# Run tests for a specific file
baml test --file notifications.baml
```

## Available Functions

### Notifications (`notifications.baml`)

**CategorizeNotification**

- Automatically categorize notifications by content
- Determines priority and lifetime
- Input: title, message, source context
- Output: category, priority, suggested lifetime, reasoning

**SummarizeNotifications**

- Generate daily/weekly notification digests
- Input: list of notifications, time period
- Output: summary, key actions, trends, recommendations

**GroupRelatedNotifications**

- Suggest intelligent grouping of related notifications
- Input: map of notification IDs to content
- Output: suggested groups with reasoning

**IsNotificationStillRelevant**

- Determine if old notifications should be archived
- Input: notification details, age, current system state
- Output: boolean indicating relevance

### Questions (`questions.baml`)

**GenerateSystemQuestion**

- Create context-aware questions for ask service
- Input: check type, system state, user context, history
- Output: question text, suggested responses, follow-up actions

**AnalyzeAnswer**

- Interpret user's answer to extract intent
- Input: original question, user answer, expected response type
- Output: interpretation, confidence, suggested action

**GenerateFollowUpQuestion**

- Create clarifying follow-up questions
- Input: original question/answer, ambiguity reason
- Output: follow-up question with updated context

**EvaluateQuestionEffectiveness**

- Assess question quality for continuous improvement
- Input: question, response time, answer, outcome
- Output: effectiveness feedback and improvements

**PersonalizeQuestion**

- Adapt questions to user's communication style
- Input: base question, user preferences, history
- Output: personalized question

### Monitoring (`monitoring.baml`)

**AnalyzeLogs**

- Comprehensive log analysis for errors and issues
- Input: service name, log entries, time window
- Output: error summary, severity, root cause, actions

**DetectLogPatterns**

- Identify recurring patterns in logs
- Input: log entries, time window
- Output: detected patterns with occurrence counts

**AssessServiceHealth**

- Overall health assessment of a service
- Input: service name, logs, metrics, expected behavior
- Output: health status, issues, recommendations

**DetectPerformanceAnomaly**

- Find metrics deviating from normal patterns
- Input: metric name, current/historical values, baseline
- Output: anomaly details and investigation steps (or null if normal)

**CorrelateServiceErrors**

- Find related failures across services
- Input: map of service errors, time window
- Output: correlation analysis and root cause hypothesis

### CLI (`cli.baml`)

**ParseNaturalLanguageCommand**

- Convert natural language to CLI commands
- Input: user input, current context
- Output: action, parameters, confidence, confirmation needs

**SuggestCommandCorrection**

- Suggest corrections for failed commands
- Input: user input, error message
- Output: corrected command, explanation, alternatives

**ProvideNaturalLanguageHelp**

- Answer questions about CLI usage
- Input: user question, available commands
- Output: answer, examples, related topics

**ExplainCommand**

- Explain what a command will do before execution
- Input: command string, current context
- Output: clear explanation of effects and reversibility

**SuggestCommandAliases**

- Suggest helpful aliases based on usage patterns
- Input: command history, frequency map
- Output: map of suggested aliases to full commands

### Hooks (`hooks.baml`)

**AnalyzeShellEvent**

- Decide if shell command needs notification
- Input: command, exit code, output, duration, context
- Output: notification decision with title, message, priority

**LearnCommandPatterns**

- Learn patterns from command history
- Input: command history, notification history
- Output: learned patterns with notification rules

**AnalyzeCommandIntent**

- Understand command before execution
- Input: command, execution context
- Output: purpose, long-running prediction, expected outcomes

**GenerateCompletionNotification**

- Create notification content for completed commands
- Input: command, exit code, duration, output, attempts
- Output: notification title and message

**FilterRelevantOutput**

- Extract important lines from verbose output
- Input: full output, exit code, max lines
- Output: filtered relevant output

## Client Configuration

### Primary Clients

**LocalOllama** (Default)

- Model: llama3.2 (configurable)
- URL: http://localhost:11434/v1
- No API key needed
- Privacy: All processing local

**LocalOllamaFast**

- Model: qwen2.5:3b (smaller, faster)
- For low-latency operations

**AgentdPrimary** (Recommended)

- Fallback strategy: LocalOllama → CustomHaiku (Claude)
- Automatic failover if Ollama unavailable

**AgentdFast**

- Fallback strategy: LocalOllamaFast → CustomHaiku
- For time-sensitive operations

### Switching Models

Edit `clients.baml` to change models:

```baml
client<llm> LocalOllama {
  provider openai-generic
  options {
    base_url "http://localhost:11434/v1"
    model "llama3.2"  // Change this to: llama4, qwen2.5, mistral, etc.
    default_role "user"
  }
}
```

Available Ollama models:

- `llama3.2` - Good balance of speed and quality
- `qwen2.5` - Fast and efficient
- `mistral` - Strong reasoning capabilities
- See: https://ollama.ai/library

### Cloud Fallback

To enable cloud fallback (optional):

```bash
# For Anthropic Claude
export ANTHROPIC_API_KEY=your_key_here

# Functions will use LocalOllama first, Claude if Ollama fails
```

## Integration with Rust Services

### Rust Client Crate

BAML integration uses a dedicated Rust crate (`crates/baml`) that wraps the REST API:

```rust
use baml::{BamlClient, BamlClientConfig};

// Create client
let client = BamlClient::default();

// Call BAML function
let result = client.categorize_notification(
    "Database Error",
    "Connection failed",
    "production"
).await?;
```

See `crates/baml/README.md` for complete API documentation.

### Architecture

```
┌─────────────────┐
│  Rust Services  │
│  (agentd-*)     │
└────────┬────────┘
         │ uses
         ▼
┌─────────────────┐      HTTP/REST      ┌─────────────┐
│  baml crate     ├──────────────────────► BAML Server │
│  (Rust client)  │                      │ (baml serve)│
└─────────────────┘                      └──────┬──────┘
                                                 │ loads
                                                 ▼
                                         ┌─────────────┐
                                         │  baml_src/  │
                                         │  (*.baml)   │
                                         └─────────────┘
                                                 │ calls
                                                 ▼
                                         ┌─────────────┐
                                         │   Ollama    │
                                         │  (local LLM)│
                                         └─────────────┘
```

### Setup

1. **Start BAML Server** (one-time, keep running):

   ```bash
   baml serve
   ```

2. **Use in Rust** (from any service):

   ```rust
   use baml::BamlClient;

   let baml = BamlClient::default();
   let result = baml.categorize_notification(...).await?;
   ```

## Usage Examples

### Example 1: Categorize a Notification

**Rust:**

```rust
use baml::BamlClient;

let client = BamlClient::default();

let result = client.categorize_notification(
    "Database Connection Lost",
    "Unable to connect to PostgreSQL",
    "production monitoring"
).await?;

println!("Category: {}", result.category);
println!("Priority: {}", result.priority);
println!("Reasoning: {}", result.reasoning);
```

**Expected Output:**

```
Category: urgent
Priority: urgent
Lifetime: persistent
Reasoning: Database connection failure affects all services and requires immediate attention...
```

### Example 2: Generate Smart Question

**Rust:**

```rust
let result = client.generate_system_question(
    "tmux_sessions",
    "0 tmux sessions running",
    "User is in terminal, last session ended 2 hours ago",
    "User typically runs 2-3 tmux sessions. Previous: 'dev-main'"
).await?;

println!("Question: {}", result.question_text);
println!("Suggestions: {:?}", result.suggested_responses);
```

**Expected Output:**

```
Question: "No tmux sessions are running. Would you like to start your usual development environment?"
Suggestions: ["Yes, start dev-main", "No, not right now", "Start a new session"]
Urgency: normal
```

### Example 3: Analyze Logs

**Rust:**

```rust
let logs = vec![
    "[ERROR] Failed to connect to database".to_string(),
    "[ERROR] Retry attempt 1 failed".to_string(),
    "[WARN] Falling back to in-memory storage".to_string(),
];

let result = client.analyze_logs(
    "agentd-notify",
    &logs,
    "last 5 minutes"
).await?;

println!("Has Errors: {}", result.has_errors);
println!("Severity: {}", result.severity);
println!("Summary: {}", result.error_summary);
```

See `crates/baml/examples/` for complete working examples:

```bash
# Start BAML server first
baml serve

# Then run example
cargo run --example categorize_notification
```

## Performance Considerations

### Latency

- **Local Ollama**: 100-500ms per request

  - llama3.2: ~200-300ms
  - qwen2.5:3b: ~100-150ms (faster model)

- **Cloud Fallback**: 200-800ms per request
  - Claude Haiku: ~200-400ms
  - Higher latency but guaranteed availability

### Token Usage (Cloud Models)

Estimated tokens per function:

- CategorizeNotification: ~200 tokens
- GenerateSystemQuestion: ~300 tokens
- AnalyzeLogs: ~500-1000 tokens (depends on log count)
- SummarizeNotifications: ~800-1500 tokens

**Cost estimates** (Claude Haiku):

- ~$0.25 per 1M input tokens
- Typical usage: <1000 calls/day = <$0.25/month
- Local Ollama is free

### Caching

For repeated similar requests, consider implementing response caching:

```python
# Example: Cache notification categories
from functools import lru_cache

@lru_cache(maxsize=100)
async def cached_categorize(title: str, message: str):
    return await b.CategorizeNotification(title, message, "cache")
```

## Troubleshooting

### BAML Check Fails

```bash
baml check --display-all-warnings
```

Common issues:

- Syntax errors in prompts (check quote escaping)
- Invalid field types in classes
- Undefined client references

### Generation Fails

```bash
baml generate --no-version-check
```

Issues:

- Version mismatch: Update BAML CLI or `generators.baml` version
- Output directory permissions

### Ollama Not Responding

```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# Start Ollama
ollama serve

# Pull model if not present
ollama pull llama3.2
```

### Function Returns Poor Results

1. **Check prompt quality**: Review function prompts in `.baml` files
2. **Try different model**: Edit `clients.baml` to use stronger model
3. **Add more context**: Provide more detailed input parameters
4. **Review test outputs**: Run `baml test` to see example outputs

### Performance Issues

1. **Use faster model**: Switch to qwen2.5:3b for speed
2. **Reduce prompt size**: Limit input array sizes
3. **Parallel requests**: BAML supports concurrent function calls
4. **Implement caching**: Cache results for repeated requests

## Development Best Practices

### Adding New Functions

1. Define data models (classes) first
2. Write function signature with clear parameter types
3. Craft detailed prompt with examples and guidelines
4. Write at least 2 test cases (success + edge case)
5. Run `baml check` and fix any warnings
6. Test with `baml test`
7. Generate clients with `baml generate`

### Prompt Engineering Tips

1. **Be specific**: Clear instructions yield better results
2. **Provide examples**: Show desired output format
3. **Set constraints**: Define valid values, ranges, formats
4. **Handle edge cases**: Address ambiguous or missing inputs
5. **Use context variables**: `{{ ctx.output_format }}` ensures correct structure
6. **Test iteratively**: Use `baml repl` for rapid iteration

### Testing Strategy

1. **Unit tests**: BAML built-in tests for each function
2. **Integration tests**: Test with real service data
3. **Edge cases**: Test with empty inputs, errors, edge values
4. **Performance tests**: Measure latency for different models

## Next Steps

### Phase 1: Current Status ✓

- [x] BAML project initialized
- [x] Clients configured (Ollama + Claude fallback)
- [x] Functions defined for all use cases
- [x] Python client generated
- [x] Documentation created

### Phase 2: Integration (Next)

- [ ] Start BAML server (`baml serve`)
- [ ] Create Rust client wrapper in `crates/ollama`
- [ ] Integrate with agentd-notify service
- [ ] Test notification categorization
- [ ] Benchmark performance

### Phase 3: Rollout

- [ ] Integrate with agentd-ask for smart questions
- [ ] Add log analysis to agentd-monitor
- [ ] Enable natural language CLI parsing
- [ ] Implement hook intelligence
- [ ] Collect user feedback

### Phase 4: Optimization

- [ ] Fine-tune prompts based on real usage
- [ ] Implement response caching
- [ ] Add monitoring and metrics
- [ ] Optimize for latency
- [ ] Document production deployment

## Resources

### Documentation

- BAML Docs: https://docs.boundaryml.com/home
- GitHub: https://github.com/BoundaryML/baml
- VSCode Extension: Search "BAML" in extensions

### Related Files

- Research doc: `../docs/research/baml-investigation.md`
- Main README: `../README.md`
- Ollama crate: `../crates/ollama/`

### Support

- GitHub Issues: https://github.com/BoundaryML/baml/issues
- Internal: See `docs/research/baml-investigation.md`

---

**Last Updated:** 2025-11-08
**BAML Version:** 0.213.0
**Status:** Phase 1 Complete, Ready for Integration
