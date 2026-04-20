# Prompt Function Library

This directory contains reusable, structured prompt definitions that can serve as
building blocks for agentd workflows and services. Each YAML file defines a set of
typed prompt functions organized by domain.

## Background

These prompts were originally defined using [BAML](https://docs.boundaryml.com/) -
a DSL for typed LLM function calls. BAML was removed from the project because:

- It required running a separate server process (`baml serve`) alongside the Rust services.
- BAML lacks native Rust code generation, so integration was via a hand-written HTTP
  client calling the BAML server's REST API - losing BAML's main value proposition.
- The same structured-output capabilities are now available natively through Claude's
  tool_use and response schemas, without an intermediary service.

The prompt engineering, type definitions, and test cases represent real design work
and are preserved here as the foundation for a native prompt function system.

## Files

| File | Domain | Functions | Description |
|------|--------|-----------|-------------|
| `notifications.yml` | Notify service | 4 | Categorize, summarize, group, and filter notifications |
| `questions.yml` | Ask service | 5 | Generate contextual questions, analyze answers, evaluate effectiveness |
| `monitoring.yml` | Monitor service | 5 | Log analysis, pattern detection, health assessment, anomaly detection |
| `cli.yml` | CLI | 5 | Natural language command parsing, correction, help, aliases |
| `hooks.yml` | Hook service | 5 | Shell event analysis, pattern learning, notification generation |

## YAML Structure

Each file follows this structure:

```yaml
functions:
  - name: FunctionName
    description: What this function does
    inputs:
      - name: param_name
        type: string          # string, int, float, bool, string[], map<K,V>
        description: ...
    output:
      type: OutputTypeName    # Named type or primitive
      schema:                 # Field definitions for structured types
        field_name:
          type: string
          enum: [a, b, c]     # Optional: constrained values
          optional: true       # Optional: nullable field
          description: ...
    prompt: |
      The prompt template with {{variable}} placeholders.
    client: primary           # LLM client tier (see below)
    tests:                    # Example inputs for validation
      - name: test_name
        inputs:
          param: value
```

## Client Tiers

The original BAML definitions used a client routing strategy that maps to these
logical tiers. An implementation should allow configuring which model backs each tier:

| Tier | Original BAML Client | Intent |
|------|---------------------|--------|
| `primary` | `AgentdPrimary` | Complex reasoning tasks; falls back from local to cloud |
| `fast` | `AgentdFast` | Low-latency tasks; smaller/faster models preferred |
| `local_primary` | `LocalOllama` | Tasks that should stay local (privacy, cost, latency) |

## Intended Use: Native Prompt Functions

These definitions are designed to inform a native agentd feature - lightweight,
typed LLM calls that the orchestrator can execute without spinning up a full agent.
This fills the gap between the current workflow system's two extremes:

- **Full agent dispatch**: Long-running Claude Code session with tools (heavy)
- **Static template rendering**: `{{variable}}` substitution with no intelligence (no LLM)

### The Missing Middle

A prompt function is a single LLM call with typed inputs, a prompt template, and a
typed output schema. The orchestrator executes it directly via the Claude API (or any
configured provider) using structured output (tool_use / response schema).

### Workflow Integration Concept

Prompt functions could integrate into the workflow system as pre-dispatch, routing,
or post-dispatch steps:

```yaml
# Hypothetical workflow with prompt function steps
name: smart-issue-triage
agent: worker

source:
  type: github_issues
  owner: myorg
  repo: myrepo
  labels: [needs-triage]

# Pre-dispatch: run a prompt function to classify before dispatching
pre_dispatch:
  function: monitoring.AssessServiceHealth   # reference to prompt function
  inputs:
    service_name: "{{source}}"
    recent_logs: "{{metadata.logs}}"
    metrics: "{{metadata.metrics}}"
    expected_behavior: "Error rate < 1%, response time < 200ms"
  # Route based on structured output
  routing:
    - when: output.status == "critical"
      agent: oncall-responder
      priority: urgent
    - when: output.status == "degraded"
      agent: worker
      priority: high
    - default:
      skip: true   # healthy, no dispatch needed

prompt_template: |
  Service {{title}} needs attention.
  Health status: {{pre_dispatch.output.status}}
  Issues: {{pre_dispatch.output.issues}}
  Recommendations: {{pre_dispatch.output.recommendations}}

  Investigate and resolve the issues described above.
```

### Implementation Considerations

When building native prompt functions, consider:

1. **Execution**: The orchestrator already manages LLM interactions through the agent
   SDK. Adding a "call LLM with structured output, no tools" path is a natural
   extension. Use Claude's tool_use or JSON mode for output parsing.

2. **Type validation**: The `output.schema` definitions in these YAML files include
   types, enums, and optional markers. A runtime validator should check LLM responses
   against the schema before passing results to routing logic.

3. **Template rendering**: Prompt templates use `{{variable}}` placeholders (same as
   workflow templates) plus Jinja-style `{% for %}` loops for array inputs. The
   existing `scheduler::template` module handles the simple case; array iteration
   would need to be added.

4. **Client routing**: The `client` field maps to a logical tier, not a specific
   model. Configuration should allow mapping tiers to providers/models, e.g.:
   ```toml
   [prompt_functions.clients]
   primary = "claude-sonnet-4-6"
   fast = "claude-haiku-4-5"
   local_primary = "ollama/llama3"
   ```

5. **Composability**: Prompt function outputs should be usable as inputs to other
   prompt functions or as variables in workflow templates. This enables pipelines
   like: categorize -> route -> dispatch -> evaluate.

6. **Testing**: Each function includes test cases with example inputs. These can
   drive integration tests that verify the full pipeline (render template, call
   LLM, validate output schema) without needing to assert on LLM output content.

7. **User-defined functions**: Beyond the built-in library, users should be able
   to define their own prompt functions in workflow YAML or in separate files,
   following the same schema. This makes the system extensible without requiring
   Rust code changes.

### Relationship to Existing Systems

| System | Role | Prompt Functions Fill |
|--------|------|---------------------|
| Workflow templates | Variable substitution into prose prompts | No intelligence, just string replacement |
| Agent dispatch | Full Claude Code session with tools | Too heavy for simple classification/routing |
| **Prompt functions** | Single typed LLM call | Lightweight intelligence for decision points |

Prompt functions complement both systems. They add intelligence to the dispatch
pipeline without the overhead of a full agent session.
