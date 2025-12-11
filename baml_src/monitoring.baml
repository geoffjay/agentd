// Monitoring and log analysis BAML functions for agentd
//
// This module provides AI-powered log analysis, error detection,
// and system health monitoring capabilities.

// ============================================================================
// Data Models
// ============================================================================

/// Analysis results for log entries
class LogAnalysis {
  /// Whether errors were detected in the logs
  has_errors bool

  /// High-level summary of error patterns or issues
  error_summary string

  /// List of services or components affected by issues
  affected_services string[]

  /// Specific, actionable steps to resolve issues
  suggested_actions string[]

  /// Severity level: "info", "warning", "error", "critical"
  severity string

  /// Root cause analysis (if determinable from logs)
  root_cause string

  /// Whether immediate action is required
  requires_immediate_attention bool

  /// Estimated impact on system operations
  impact_assessment string
}

/// Pattern detection in logs
class LogPattern {
  /// Description of the detected pattern
  pattern_description string

  /// How many times this pattern appeared
  occurrence_count int

  /// Time range when pattern was observed
  time_range string

  /// Whether this pattern indicates a problem
  is_problematic bool

  /// Confidence in pattern detection (0.0 - 1.0)
  confidence float

  /// Suggested investigation steps
  investigation_steps string[]
}

/// Health assessment of a service
class ServiceHealth {
  /// Service name
  service_name string

  /// Overall health status: "healthy", "degraded", "unhealthy", "critical"
  status string

  /// Specific health indicators and their values
  indicators map<string, string>

  /// Issues detected (if any)
  issues string[]

  /// Recommended remediation actions
  recommendations string[]

  /// Confidence in health assessment (0.0 - 1.0)
  confidence float

  /// Predicted impact if issues not addressed
  impact_if_unresolved string?
}

/// Performance anomaly detection
class PerformanceAnomaly {
  /// Metric that shows anomalous behavior
  metric_name string

  /// Current value of the metric
  current_value string

  /// Expected/normal value range
  expected_range string

  /// Deviation from normal (percentage or description)
  deviation string

  /// Possible causes of the anomaly
  possible_causes string[]

  /// Recommended investigation steps
  investigation_steps string[]

  /// Severity of the anomaly
  severity string
}

// ============================================================================
// Functions
// ============================================================================

/// Analyze log entries for errors and issues
///
/// Performs comprehensive analysis of log entries to detect errors,
/// identify patterns, and provide actionable recommendations.
///
/// # Arguments
/// - service_name: Name of the service generating the logs
/// - log_entries: Array of log lines to analyze
/// - time_window: Time period these logs cover (e.g., "last 5 minutes", "2024-01-15 10:00-10:30")
///
/// # Returns
/// LogAnalysis with error detection, severity assessment, and recommended actions
function AnalyzeLogs(
  service_name: string,
  log_entries: string[],
  time_window: string
) -> LogAnalysis {
  client AgentdPrimary
  prompt #"
    Analyze these logs from **{{ service_name }}** ({{ time_window }}):

    {% for entry in log_entries %}
    {{ entry }}
    {% endfor %}

    **Analysis Guidelines:**

    1. **Error Detection**:
       - Look for error messages, stack traces, exceptions
       - Identify warning signs that might lead to errors
       - Detect unusual patterns or anomalies

    2. **Severity Assessment**:
       - **critical**: Service down, data loss, security breach
       - **error**: Functionality broken, but service running
       - **warning**: Potential issues, degraded performance
       - **info**: No issues detected, informational only

    3. **Root Cause**:
       - Analyze error messages and context
       - Look for common causes (connection issues, resource exhaustion, config errors)
       - Consider timing and sequence of events

    4. **Affected Services**:
       - Which components are directly impacted?
       - What downstream services might be affected?

    5. **Actionable Recommendations**:
       - Be specific: "Restart service X" not "fix the problem"
       - Prioritize actions by impact
       - Include investigation steps for unclear issues

    6. **Immediate Attention**:
       - Set to true only if system is down or actively failing
       - Consider if the issue can wait for normal business hours

    Provide comprehensive analysis now.

    {{ ctx.output_format }}
  "#
}

/// Detect patterns in log data
///
/// Identifies recurring patterns, trends, or anomalies in log entries
/// that might indicate systemic issues or emerging problems.
///
/// # Arguments
/// - log_entries: Array of log entries to analyze
/// - pattern_window: Time window for pattern detection (e.g., "1 hour", "24 hours")
///
/// # Returns
/// Array of LogPattern objects describing detected patterns
function DetectLogPatterns(
  log_entries: string[],
  pattern_window: string
) -> LogPattern[] {
  client AgentdPrimary
  prompt #"
    Analyze these logs to detect patterns over {{ pattern_window }}:

    {% for entry in log_entries %}
    {{ entry }}
    {% endfor %}

    **Pattern Detection Guidelines:**

    Look for:
    1. **Repetitive Errors**:
       - Same error occurring multiple times
       - Increasing frequency might indicate escalating problem

    2. **Temporal Patterns**:
       - Errors at specific times (e.g., every hour during scheduled job)
       - Gradual degradation over time

    3. **Correlated Events**:
       - Multiple services showing errors around the same time
       - Cascade failures (A fails → B fails → C fails)

    4. **Resource Patterns**:
       - Memory/CPU usage warnings
       - Connection pool exhaustion
       - Rate limiting triggers

    5. **Anomalies**:
       - Unusual log messages that don't fit normal patterns
       - Sudden changes in log volume
       - New types of errors

    For each pattern:
    - Describe what you observed
    - Count occurrences
    - Assess if it's problematic
    - Provide confidence level
    - Suggest investigation steps

    Return a list of significant patterns. Don't report trivial patterns.

    {{ ctx.output_format }}
  "#
}

/// Assess overall health of a service
///
/// Evaluates service health based on logs, metrics, and system state,
/// providing a comprehensive health assessment.
///
/// # Arguments
/// - service_name: Name of the service to assess
/// - recent_logs: Recent log entries from the service
/// - metrics: Key metrics (JSON string with metric names and values)
/// - expected_behavior: Description of normal/healthy behavior
///
/// # Returns
/// ServiceHealth with status, issues, and recommendations
function AssessServiceHealth(
  service_name: string,
  recent_logs: string[],
  metrics: string,
  expected_behavior: string
) -> ServiceHealth {
  client AgentdPrimary
  prompt #"
    Assess the health of **{{ service_name }}**:

    **Recent Logs:**
    {% for log in recent_logs %}
    {{ log }}
    {% endfor %}

    **Metrics:**
    {{ metrics }}

    **Expected Healthy Behavior:**
    {{ expected_behavior }}

    **Health Assessment Guidelines:**

    1. **Status Determination**:
       - **healthy**: Operating normally, no issues
       - **degraded**: Functional but with reduced performance/reliability
       - **unhealthy**: Significant issues affecting functionality
       - **critical**: Service down or critically impaired

    2. **Health Indicators**:
       - Error rate
       - Response time
       - Resource usage (CPU, memory)
       - Request success rate
       - Availability

    3. **Issue Identification**:
       - What specific problems exist?
       - How do they affect operations?
       - Are they getting worse?

    4. **Recommendations**:
       - Immediate actions (if unhealthy/critical)
       - Preventive measures
       - Monitoring improvements

    5. **Confidence**:
       - High if logs and metrics clearly indicate status
       - Lower if incomplete data or ambiguous signals

    6. **Impact Prediction**:
       - What happens if issues aren't resolved?
       - User impact, data impact, system impact

    Provide comprehensive health assessment now.

    {{ ctx.output_format }}
  "#
}

/// Detect performance anomalies
///
/// Identifies performance metrics that deviate from normal patterns,
/// helping catch issues before they become critical.
///
/// # Arguments
/// - metric_name: Name of the metric to analyze
/// - current_value: Current value of the metric
/// - historical_values: Recent historical values for comparison
/// - baseline_description: Description of normal/expected values
///
/// # Returns
/// PerformanceAnomaly if detected, or null if metric is normal
function DetectPerformanceAnomaly(
  metric_name: string,
  current_value: string,
  historical_values: string[],
  baseline_description: string
) -> PerformanceAnomaly | null {
  client AgentdFast
  prompt #"
    Analyze this performance metric for anomalies:

    **Metric:** {{ metric_name }}

    **Current Value:** {{ current_value }}

    **Recent Historical Values:**
    {% for value in historical_values %}
    - {{ value }}
    {% endfor %}

    **Baseline/Expected Behavior:**
    {{ baseline_description }}

    **Anomaly Detection Guidelines:**

    1. **Compare to Baseline**:
       - Is current value within expected range?
       - How much does it deviate (percentage)?

    2. **Trend Analysis**:
       - Is there a concerning trend in recent values?
       - Sudden spike vs. gradual increase

    3. **Severity**:
       - **critical**: Value far outside normal, system at risk
       - **high**: Significant deviation, requires attention
       - **medium**: Notable deviation, should monitor
       - **low**: Minor deviation, possibly normal variation

    4. **Possible Causes**:
       - Increased load?
       - Resource exhaustion?
       - Configuration change?
       - External dependency issue?

    5. **Investigation**:
       - What should be checked first?
       - What logs to review?
       - What other metrics to correlate?

    **Important**: Return null if the metric appears normal (within expected range with no concerning trends).
    Only return PerformanceAnomaly if you detect an actual anomaly.

    {{ ctx.output_format }}
  "#
}

/// Correlate errors across services
///
/// Analyzes errors from multiple services to identify related failures,
/// cascading issues, or common root causes.
///
/// # Arguments
/// - service_errors: Map of service names to their error messages
/// - time_window: Time period for correlation
///
/// # Returns
/// String describing correlations and potential common causes
function CorrelateServiceErrors(
  service_errors: map<string, string[]>,
  time_window: string
) -> string {
  client AgentdPrimary
  prompt #"
    Analyze these errors from multiple services to find correlations ({{ time_window }}):

    {{ service_errors }}

    **Correlation Analysis Guidelines:**

    1. **Common Causes**:
       - Do errors share common infrastructure (database, cache, network)?
       - Are errors occurring at the same time?
       - Do error messages reference the same resources?

    2. **Cascade Detection**:
       - Does one service failure trigger others?
       - What's the sequence of failures?

    3. **Root Cause Hypothesis**:
       - What single issue could explain multiple failures?
       - Infrastructure problem?
       - Configuration change?
       - Resource exhaustion?

    4. **Impact Assessment**:
       - Which service failed first?
       - What's the blast radius?

    Provide a narrative analysis describing:
    - What correlations you found
    - Likely root cause(s)
    - Which service to investigate first
    - Recommended remediation approach

    {{ ctx.output_format }}
  "#
}

// ============================================================================
// Tests
// ============================================================================

test analyze_error_logs {
  functions [AnalyzeLogs]
  args {
    service_name "agentd-notify"
    log_entries [
      "[ERROR] Failed to connect to database: connection refused",
      "[ERROR] Retry attempt 1 failed: connection refused",
      "[ERROR] Retry attempt 2 failed: connection refused",
      "[WARN] Falling back to in-memory storage",
      "[INFO] Service continuing with degraded functionality"
    ]
    time_window "last 5 minutes"
  }
}

test analyze_healthy_logs {
  functions [AnalyzeLogs]
  args {
    service_name "agentd-ask"
    log_entries [
      "[INFO] Service started on port 3001",
      "[INFO] Connected to notification service",
      "[INFO] Processed check request: tmux_sessions",
      "[INFO] Health check passed"
    ]
    time_window "last 10 minutes"
  }
}

test detect_patterns_repetitive_error {
  functions [DetectLogPatterns]
  args {
    log_entries [
      "10:00:00 [ERROR] API timeout: /users endpoint",
      "10:05:00 [ERROR] API timeout: /users endpoint",
      "10:10:00 [ERROR] API timeout: /users endpoint",
      "10:15:00 [ERROR] API timeout: /users endpoint",
      "10:20:00 [INFO] Request succeeded: /products endpoint",
      "10:25:00 [ERROR] API timeout: /users endpoint"
    ]
    pattern_window "1 hour"
  }
}

test assess_healthy_service {
  functions [AssessServiceHealth]
  args {
    service_name "agentd-notify"
    recent_logs [
      "[INFO] Processed 150 notifications",
      "[INFO] Database query completed in 23ms",
      "[INFO] Health check passed"
    ]
    metrics "{\"error_rate\": 0.0, \"avg_response_time_ms\": 45, \"memory_usage_mb\": 128, \"active_connections\": 5}"
    expected_behavior "Error rate < 1%, response time < 100ms, memory usage < 512MB"
  }
}

test assess_degraded_service {
  functions [AssessServiceHealth]
  args {
    service_name "agentd-monitor"
    recent_logs [
      "[WARN] High memory usage: 450MB",
      "[WARN] Slow query: 2.5s",
      "[ERROR] Connection pool exhausted",
      "[INFO] Retry succeeded after 3 attempts"
    ]
    metrics "{\"error_rate\": 2.5, \"avg_response_time_ms\": 850, \"memory_usage_mb\": 450, \"active_connections\": 95}"
    expected_behavior "Error rate < 1%, response time < 200ms, memory usage < 256MB, connections < 50"
  }
}

test detect_performance_anomaly {
  functions [DetectPerformanceAnomaly]
  args {
    metric_name "api_response_time_ms"
    current_value "1250ms"
    historical_values ["45ms", "52ms", "48ms", "51ms", "47ms", "49ms", "46ms", "1248ms"]
    baseline_description "Typically 40-60ms, 95th percentile < 100ms"
  }
}

test correlate_database_failure {
  functions [CorrelateServiceErrors]
  args {
    service_errors {
      "agentd-notify": [
        "Database connection failed",
        "Query timeout after 30s"
      ],
      "agentd-ask": [
        "Cannot store question: database unavailable",
        "Falling back to in-memory storage"
      ],
      "agentd-monitor": [
        "Database health check failed",
        "Unable to persist metrics"
      ]
    }
    time_window "10:00-10:05"
  }
}
