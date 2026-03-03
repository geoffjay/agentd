//! BAML client for making requests to the BAML server.

use crate::error::{BamlError, Result};
use crate::types::*;
use reqwest::{Client as HttpClient, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tracing::debug;

/// Configuration for the BAML client
#[derive(Debug, Clone)]
pub struct BamlClientConfig {
    /// Base URL of the BAML server (e.g., "http://localhost:2024")
    pub base_url: String,
    /// Timeout for requests in seconds (default: 30)
    pub timeout_secs: u64,
    /// Number of retries for failed requests (default: 2)
    pub max_retries: u32,
}

impl Default for BamlClientConfig {
    fn default() -> Self {
        Self { base_url: "http://localhost:2024".to_string(), timeout_secs: 30, max_retries: 2 }
    }
}

impl BamlClientConfig {
    /// Create a new config with the given base URL
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), ..Default::default() }
    }

    /// Set the timeout in seconds
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    /// Set the maximum number of retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// Client for interacting with BAML server functions
///
/// The BAML server must be running (via `baml serve`) for this client to work.
///
/// # Examples
///
/// ```no_run
/// use baml::{BamlClient, BamlClientConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = BamlClientConfig::new("http://localhost:2024");
///     let client = BamlClient::new(config);
///
///     let result = client.categorize_notification(
///         "Database Error",
///         "Connection failed",
///         "production"
///     ).await?;
///
///     println!("Category: {}", result.category);
///     Ok(())
/// }
/// ```
pub struct BamlClient {
    config: BamlClientConfig,
    http_client: HttpClient,
}

impl Default for BamlClient {
    fn default() -> Self {
        Self::new(BamlClientConfig::default())
    }
}

impl BamlClient {
    /// Create a new BAML client with the given configuration
    pub fn new(config: BamlClientConfig) -> Self {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to build HTTP client");

        Self { config, http_client }
    }

    /// Call a BAML function with the given parameters
    async fn call_function<P, R>(&self, function_name: &str, params: &P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let url = format!("{}/call/{}", self.config.base_url, function_name);
        debug!("Calling BAML function: {}", function_name);

        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                debug!("Retry attempt {} for {}", attempt, function_name);
            }

            match self.http_client.post(&url).json(params).send().await {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        let response_text = response.text().await?;
                        let result: R = serde_json::from_str(&response_text).map_err(|e| {
                            BamlError::InvalidResponse(format!(
                                "Failed to parse response: {}. Response was: {}",
                                e, response_text
                            ))
                        })?;
                        return Ok(result);
                    } else if status == StatusCode::NOT_FOUND {
                        return Err(BamlError::FunctionNotFound {
                            function_name: function_name.to_string(),
                        });
                    } else {
                        let error_body = response.text().await.unwrap_or_default();
                        last_error = Some(BamlError::ServerError {
                            status: status.as_u16(),
                            message: error_body,
                        });
                    }
                }
                Err(e) if e.is_timeout() => {
                    last_error =
                        Some(BamlError::Timeout { timeout_secs: self.config.timeout_secs });
                }
                Err(e) if e.is_connect() => {
                    last_error = Some(BamlError::ServerUnreachable { url: url.clone(), source: e });
                }
                Err(e) => {
                    last_error = Some(BamlError::RequestFailed(e));
                }
            }

            if attempt < self.config.max_retries {
                tokio::time::sleep(Duration::from_millis(100 * (attempt as u64 + 1))).await;
            }
        }

        Err(last_error.unwrap_or_else(|| BamlError::InvalidResponse("Unknown error".to_string())))
    }

    // ========================================================================
    // Notification Functions
    // ========================================================================

    /// Categorize a notification automatically based on its content
    ///
    /// # Arguments
    /// * `title` - The notification title
    /// * `message` - The notification message body
    /// * `source_context` - Context about where the notification came from
    ///
    /// # Example
    /// ```no_run
    /// # use baml::BamlClient;
    /// # async fn example(client: BamlClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let result = client.categorize_notification(
    ///     "Database Connection Lost",
    ///     "Unable to connect to PostgreSQL",
    ///     "production monitoring"
    /// ).await?;
    /// println!("Priority: {}", result.priority);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn categorize_notification(
        &self,
        title: &str,
        message: &str,
        source_context: &str,
    ) -> Result<NotificationCategory> {
        #[derive(Serialize)]
        struct Params<'a> {
            title: &'a str,
            message: &'a str,
            source_context: &'a str,
        }

        self.call_function("CategorizeNotification", &Params { title, message, source_context })
            .await
    }

    /// Generate a digest summarizing multiple notifications
    ///
    /// # Arguments
    /// * `notifications` - List of notification summaries
    /// * `time_period` - Description of the time period (e.g., "last 24 hours")
    pub async fn summarize_notifications(
        &self,
        notifications: &[String],
        time_period: &str,
    ) -> Result<NotificationDigest> {
        #[derive(Serialize)]
        struct Params<'a> {
            notifications: &'a [String],
            time_period: &'a str,
        }

        self.call_function("SummarizeNotifications", &Params { notifications, time_period }).await
    }

    /// Suggest intelligent grouping of related notifications
    ///
    /// # Arguments
    /// * `notifications` - Map of notification IDs to their content
    /// * `min_group_size` - Minimum number of notifications to form a group
    pub async fn group_related_notifications(
        &self,
        notifications: &HashMap<String, String>,
        min_group_size: i32,
    ) -> Result<Vec<NotificationGroup>> {
        #[derive(Serialize)]
        struct Params<'a> {
            notifications: &'a HashMap<String, String>,
            min_group_size: i32,
        }

        self.call_function("GroupRelatedNotifications", &Params { notifications, min_group_size })
            .await
    }

    /// Determine if a notification is still relevant
    ///
    /// # Arguments
    /// * `title` - Notification title
    /// * `message` - Notification message
    /// * `created_hours_ago` - How many hours ago the notification was created
    /// * `current_system_state` - Brief description of current system state
    pub async fn is_notification_still_relevant(
        &self,
        title: &str,
        message: &str,
        created_hours_ago: i32,
        current_system_state: &str,
    ) -> Result<bool> {
        #[derive(Serialize)]
        struct Params<'a> {
            title: &'a str,
            message: &'a str,
            created_hours_ago: i32,
            current_system_state: &'a str,
        }

        self.call_function(
            "IsNotificationStillRelevant",
            &Params { title, message, created_hours_ago, current_system_state },
        )
        .await
    }

    // ========================================================================
    // Question Functions
    // ========================================================================

    /// Generate a context-aware system question
    ///
    /// # Arguments
    /// * `check_type` - Type of system check (e.g., "tmux_sessions")
    /// * `system_state` - Current system state
    /// * `user_context` - Information about user's current activity
    /// * `recent_history` - Recent relevant events or user actions
    pub async fn generate_system_question(
        &self,
        check_type: &str,
        system_state: &str,
        user_context: &str,
        recent_history: &str,
    ) -> Result<SystemQuestion> {
        #[derive(Serialize)]
        struct Params<'a> {
            check_type: &'a str,
            system_state: &'a str,
            user_context: &'a str,
            recent_history: &'a str,
        }

        self.call_function(
            "GenerateSystemQuestion",
            &Params { check_type, system_state, user_context, recent_history },
        )
        .await
    }

    /// Analyze user's answer to a question
    ///
    /// # Arguments
    /// * `original_question` - The question that was asked
    /// * `user_answer` - The user's response
    /// * `expected_response_type` - Type of expected response (e.g., "yes_no")
    pub async fn analyze_answer(
        &self,
        original_question: &str,
        user_answer: &str,
        expected_response_type: &str,
    ) -> Result<AnswerAnalysis> {
        #[derive(Serialize)]
        struct Params<'a> {
            original_question: &'a str,
            user_answer: &'a str,
            expected_response_type: &'a str,
        }

        self.call_function(
            "AnalyzeAnswer",
            &Params { original_question, user_answer, expected_response_type },
        )
        .await
    }

    /// Generate a clarifying follow-up question
    ///
    /// # Arguments
    /// * `original_question` - The original question
    /// * `original_answer` - The user's ambiguous answer
    /// * `ambiguity_reason` - Why the answer needs clarification
    pub async fn generate_followup_question(
        &self,
        original_question: &str,
        original_answer: &str,
        ambiguity_reason: &str,
    ) -> Result<SystemQuestion> {
        #[derive(Serialize)]
        struct Params<'a> {
            original_question: &'a str,
            original_answer: &'a str,
            ambiguity_reason: &'a str,
        }

        self.call_function(
            "GenerateFollowUpQuestion",
            &Params { original_question, original_answer, ambiguity_reason },
        )
        .await
    }

    /// Evaluate question effectiveness for continuous improvement
    pub async fn evaluate_question_effectiveness(
        &self,
        question_text: &str,
        response_time_seconds: i32,
        user_answer: &str,
        system_outcome: &str,
    ) -> Result<QuestionFeedback> {
        #[derive(Serialize)]
        struct Params<'a> {
            question_text: &'a str,
            response_time_seconds: i32,
            user_answer: &'a str,
            system_outcome: &'a str,
        }

        self.call_function(
            "EvaluateQuestionEffectiveness",
            &Params { question_text, response_time_seconds, user_answer, system_outcome },
        )
        .await
    }

    /// Generate a personalized question based on user preferences
    pub async fn personalize_question(
        &self,
        base_question: &str,
        user_preferences: &str,
        interaction_history: &str,
    ) -> Result<SystemQuestion> {
        #[derive(Serialize)]
        struct Params<'a> {
            base_question: &'a str,
            user_preferences: &'a str,
            interaction_history: &'a str,
        }

        self.call_function(
            "PersonalizeQuestion",
            &Params { base_question, user_preferences, interaction_history },
        )
        .await
    }

    // ========================================================================
    // Monitoring Functions
    // ========================================================================

    /// Analyze log entries for errors and issues
    ///
    /// # Arguments
    /// * `service_name` - Name of the service generating the logs
    /// * `log_entries` - Array of log lines to analyze
    /// * `time_window` - Time period these logs cover
    pub async fn analyze_logs(
        &self,
        service_name: &str,
        log_entries: &[String],
        time_window: &str,
    ) -> Result<LogAnalysis> {
        #[derive(Serialize)]
        struct Params<'a> {
            service_name: &'a str,
            log_entries: &'a [String],
            time_window: &'a str,
        }

        self.call_function("AnalyzeLogs", &Params { service_name, log_entries, time_window }).await
    }

    /// Detect patterns in log data
    pub async fn detect_log_patterns(
        &self,
        log_entries: &[String],
        pattern_window: &str,
    ) -> Result<Vec<LogPattern>> {
        #[derive(Serialize)]
        struct Params<'a> {
            log_entries: &'a [String],
            pattern_window: &'a str,
        }

        self.call_function("DetectLogPatterns", &Params { log_entries, pattern_window }).await
    }

    /// Assess overall health of a service
    pub async fn assess_service_health(
        &self,
        service_name: &str,
        recent_logs: &[String],
        metrics: &str,
        expected_behavior: &str,
    ) -> Result<ServiceHealth> {
        #[derive(Serialize)]
        struct Params<'a> {
            service_name: &'a str,
            recent_logs: &'a [String],
            metrics: &'a str,
            expected_behavior: &'a str,
        }

        self.call_function(
            "AssessServiceHealth",
            &Params { service_name, recent_logs, metrics, expected_behavior },
        )
        .await
    }

    /// Detect performance anomalies
    pub async fn detect_performance_anomaly(
        &self,
        metric_name: &str,
        current_value: &str,
        historical_values: &[String],
        baseline_description: &str,
    ) -> Result<Option<PerformanceAnomaly>> {
        #[derive(Serialize)]
        struct Params<'a> {
            metric_name: &'a str,
            current_value: &'a str,
            historical_values: &'a [String],
            baseline_description: &'a str,
        }

        self.call_function(
            "DetectPerformanceAnomaly",
            &Params { metric_name, current_value, historical_values, baseline_description },
        )
        .await
    }

    /// Correlate errors across services
    pub async fn correlate_service_errors(
        &self,
        service_errors: &HashMap<String, Vec<String>>,
        time_window: &str,
    ) -> Result<String> {
        #[derive(Serialize)]
        struct Params<'a> {
            service_errors: &'a HashMap<String, Vec<String>>,
            time_window: &'a str,
        }

        self.call_function("CorrelateServiceErrors", &Params { service_errors, time_window }).await
    }

    // ========================================================================
    // CLI Functions
    // ========================================================================

    /// Parse natural language input into a structured command
    pub async fn parse_natural_language_command(
        &self,
        user_input: &str,
        current_context: &str,
    ) -> Result<CommandIntent> {
        #[derive(Serialize)]
        struct Params<'a> {
            user_input: &'a str,
            current_context: &'a str,
        }

        self.call_function("ParseNaturalLanguageCommand", &Params { user_input, current_context })
            .await
    }

    /// Suggest command corrections or completions
    pub async fn suggest_command_correction(
        &self,
        user_input: &str,
        error_message: &str,
    ) -> Result<CommandSuggestion> {
        #[derive(Serialize)]
        struct Params<'a> {
            user_input: &'a str,
            error_message: &'a str,
        }

        self.call_function("SuggestCommandCorrection", &Params { user_input, error_message }).await
    }

    /// Provide help for natural language queries
    pub async fn provide_natural_language_help(
        &self,
        user_question: &str,
        available_commands: &str,
    ) -> Result<HelpResponse> {
        #[derive(Serialize)]
        struct Params<'a> {
            user_question: &'a str,
            available_commands: &'a str,
        }

        self.call_function(
            "ProvideNaturalLanguageHelp",
            &Params { user_question, available_commands },
        )
        .await
    }

    /// Explain what a command will do before execution
    pub async fn explain_command(
        &self,
        command_string: &str,
        current_context: &str,
    ) -> Result<String> {
        #[derive(Serialize)]
        struct Params<'a> {
            command_string: &'a str,
            current_context: &'a str,
        }

        self.call_function("ExplainCommand", &Params { command_string, current_context }).await
    }

    /// Generate command aliases or shortcuts
    pub async fn suggest_command_aliases(
        &self,
        command_history: &[String],
        frequency_map: &HashMap<String, i32>,
    ) -> Result<HashMap<String, String>> {
        #[derive(Serialize)]
        struct Params<'a> {
            command_history: &'a [String],
            frequency_map: &'a HashMap<String, i32>,
        }

        self.call_function("SuggestCommandAliases", &Params { command_history, frequency_map })
            .await
    }

    // ========================================================================
    // Hook Functions
    // ========================================================================

    /// Analyze a shell event to determine if notification is needed
    pub async fn analyze_shell_event(
        &self,
        command: &str,
        exit_code: i32,
        output: &str,
        duration_ms: i32,
        context: &str,
    ) -> Result<HookAction> {
        #[derive(Serialize)]
        struct Params<'a> {
            command: &'a str,
            exit_code: i32,
            output: &'a str,
            duration_ms: i32,
            context: &'a str,
        }

        self.call_function(
            "AnalyzeShellEvent",
            &Params { command, exit_code, output, duration_ms, context },
        )
        .await
    }

    /// Learn patterns from command history
    pub async fn learn_command_patterns(
        &self,
        command_history: &[String],
        notification_history: &[String],
    ) -> Result<Vec<HookPattern>> {
        #[derive(Serialize)]
        struct Params<'a> {
            command_history: &'a [String],
            notification_history: &'a [String],
        }

        self.call_function(
            "LearnCommandPatterns",
            &Params { command_history, notification_history },
        )
        .await
    }

    /// Provide insights about a command before execution
    pub async fn analyze_command_intent(
        &self,
        command: &str,
        execution_context: &str,
    ) -> Result<CommandInsight> {
        #[derive(Serialize)]
        struct Params<'a> {
            command: &'a str,
            execution_context: &'a str,
        }

        self.call_function("AnalyzeCommandIntent", &Params { command, execution_context }).await
    }

    /// Generate smart notification content for command completion
    pub async fn generate_completion_notification(
        &self,
        command: &str,
        exit_code: i32,
        duration_ms: i32,
        output_summary: &str,
        previous_attempts: i32,
    ) -> Result<String> {
        #[derive(Serialize)]
        struct Params<'a> {
            command: &'a str,
            exit_code: i32,
            duration_ms: i32,
            output_summary: &'a str,
            previous_attempts: i32,
        }

        self.call_function(
            "GenerateCompletionNotification",
            &Params { command, exit_code, duration_ms, output_summary, previous_attempts },
        )
        .await
    }

    /// Filter out noise from command output
    pub async fn filter_relevant_output(
        &self,
        full_output: &str,
        exit_code: i32,
        max_lines: i32,
    ) -> Result<String> {
        #[derive(Serialize)]
        struct Params<'a> {
            full_output: &'a str,
            exit_code: i32,
            max_lines: i32,
        }

        self.call_function("FilterRelevantOutput", &Params { full_output, exit_code, max_lines })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::BamlError;
    use mockito::Server;

    fn client_with_url(url: &str) -> BamlClient {
        let config = BamlClientConfig::new(url).with_timeout(5).with_max_retries(0);
        BamlClient::new(config)
    }

    fn client_with_retries(url: &str, retries: u32) -> BamlClient {
        let config = BamlClientConfig::new(url).with_timeout(5).with_max_retries(retries);
        BamlClient::new(config)
    }

    // -- Config tests --

    #[test]
    fn test_default_config() {
        let config = BamlClientConfig::default();
        assert_eq!(config.base_url, "http://localhost:2024");
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 2);
    }

    #[test]
    fn test_config_builder() {
        let config =
            BamlClientConfig::new("http://example.com").with_timeout(10).with_max_retries(5);
        assert_eq!(config.base_url, "http://example.com");
        assert_eq!(config.timeout_secs, 10);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_default_client() {
        let _client = BamlClient::default();
    }

    // -- call_function success --

    #[tokio::test]
    async fn test_call_function_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/call/CategorizeNotification")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                "category": "error",
                "priority": "high",
                "suggested_lifetime": "persistent",
                "reasoning": "Database error detected",
                "suggested_action": "Check database connectivity"
            }"#,
            )
            .create_async()
            .await;

        let client = client_with_url(&server.url());
        let result =
            client.categorize_notification("DB Error", "Connection lost", "production").await;

        assert!(result.is_ok());
        let category = result.unwrap();
        assert_eq!(category.category, "error");
        assert_eq!(category.priority, "high");
        mock.assert_async().await;
    }

    // -- Error handling --

    #[tokio::test]
    async fn test_function_not_found() {
        let mut server = Server::new_async().await;
        let mock =
            server.mock("POST", "/call/NonExistentFunction").with_status(404).create_async().await;

        let client = client_with_url(&server.url());
        let result: Result<String> = client.call_function("NonExistentFunction", &"{}").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BamlError::FunctionNotFound { .. }));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_server_error_500() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/call/TestFunc")
            .with_status(500)
            .with_body("Internal server error")
            .create_async()
            .await;

        let client = client_with_url(&server.url());
        let result: Result<String> = client.call_function("TestFunc", &"{}").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BamlError::ServerError { status: 500, .. }));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_invalid_json_response() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/call/CategorizeNotification")
            .with_status(200)
            .with_body("not valid json")
            .create_async()
            .await;

        let client = client_with_url(&server.url());
        let result = client.categorize_notification("Test", "Test", "test").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BamlError::InvalidResponse(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_server_unreachable() {
        let client = client_with_url("http://127.0.0.1:19999");
        let result = client.categorize_notification("Test", "Test", "test").await;

        assert!(result.is_err());
    }

    // -- Retry logic --

    #[tokio::test]
    async fn test_retry_on_server_error() {
        let mut server = Server::new_async().await;

        // First call returns 500, second returns 200
        let fail_mock = server
            .mock("POST", "/call/TestFunc")
            .with_status(500)
            .with_body("temporary error")
            .expect(1)
            .create_async()
            .await;

        let success_mock = server
            .mock("POST", "/call/TestFunc")
            .with_status(200)
            .with_body("\"success\"")
            .expect(1)
            .create_async()
            .await;

        let client = client_with_retries(&server.url(), 1);
        let result: Result<String> = client.call_function("TestFunc", &"{}").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        fail_mock.assert_async().await;
        success_mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_no_retry_on_404() {
        let mut server = Server::new_async().await;

        let mock =
            server.mock("POST", "/call/Missing").with_status(404).expect(1).create_async().await;

        let client = client_with_retries(&server.url(), 2);
        let result: Result<String> = client.call_function("Missing", &"{}").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BamlError::FunctionNotFound { .. }));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_exhausts_retries() {
        let mut server = Server::new_async().await;

        let mock = server
            .mock("POST", "/call/TestFunc")
            .with_status(500)
            .with_body("persistent error")
            .expect(3) // initial + 2 retries
            .create_async()
            .await;

        let client = client_with_retries(&server.url(), 2);
        let result: Result<String> = client.call_function("TestFunc", &"{}").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BamlError::ServerError { .. }));
        mock.assert_async().await;
    }

    // -- Domain method tests --

    #[tokio::test]
    async fn test_summarize_notifications() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/call/SummarizeNotifications")
            .with_status(200)
            .with_body(
                r#"{
                "summary": "5 notifications in the last hour",
                "key_actions": ["Review database alerts"],
                "urgent_count": 1,
                "high_count": 2,
                "normal_count": 1,
                "low_count": 1,
                "categories": {"error": 3, "info": 2},
                "trends": "Increasing error rate",
                "recommendations": ["Check database"]
            }"#,
            )
            .create_async()
            .await;

        let client = client_with_url(&server.url());
        let notifications = vec!["Alert 1".to_string(), "Alert 2".to_string()];
        let result = client.summarize_notifications(&notifications, "last hour").await;

        assert!(result.is_ok());
        let digest = result.unwrap();
        assert_eq!(digest.urgent_count, 1);
        assert_eq!(digest.high_count, 2);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_analyze_answer() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/call/AnalyzeAnswer")
            .with_status(200)
            .with_body(
                r#"{
                "interpretation": "User agrees",
                "confidence": 0.95,
                "suggested_action": "proceed",
                "needs_followup": false,
                "followup_question": null
            }"#,
            )
            .create_async()
            .await;

        let client = client_with_url(&server.url());
        let result = client.analyze_answer("Start tmux?", "yes", "yes_no").await;

        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert_eq!(analysis.confidence, 0.95);
        assert!(!analysis.needs_followup);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_analyze_shell_event() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/call/AnalyzeShellEvent")
            .with_status(200)
            .with_body(
                r#"{
                "should_notify": true,
                "notification_title": "Build Failed",
                "notification_message": "cargo build exited with code 1",
                "priority": "high",
                "reasoning": "Build failure detected",
                "metadata": {},
                "indicates_problem": true,
                "suggested_actions": ["Fix compilation errors"]
            }"#,
            )
            .create_async()
            .await;

        let client = client_with_url(&server.url());
        let result =
            client.analyze_shell_event("cargo build", 1, "error[E0308]", 5000, "development").await;

        assert!(result.is_ok());
        let action = result.unwrap();
        assert!(action.should_notify);
        assert!(action.indicates_problem);
        mock.assert_async().await;
    }
}
