use crate::scheduler::types::Task;

/// All known template variables (top-level task fields + metadata-backed).
///
/// Top-level fields are derived directly from `Task` struct fields.
/// Metadata-backed variables are stored in `Task.metadata` and populated
/// by schedule-based triggers. Which metadata variables are present depends
/// on the trigger type:
///
/// | Variable          | Trigger types          | Description                        |
/// |-------------------|------------------------|------------------------------------|
/// | `fire_time`       | cron                   | RFC 3339 timestamp of the firing   |
/// | `cron_expression` | cron                   | The cron expression that fired     |
/// | `trigger_type`    | cron, delay            | The trigger type name              |
/// | `run_at`          | delay                  | The scheduled run-at datetime      |
/// | `workflow_id`     | delay                  | The workflow UUID                  |
pub const KNOWN_VARIABLES: &[&str] = &[
    // Top-level task fields
    "title",
    "body",
    "url",
    "labels",
    "assignee",
    "source_id",
    "metadata",
    // Metadata-backed (schedule triggers)
    "fire_time",
    "cron_expression",
    "trigger_type",
    "run_at",
    "workflow_id",
];

/// Validate a prompt template, returning any warnings or errors.
///
/// Checks for:
/// - Unknown `{{variable}}` placeholders that won't be replaced
/// - Empty template
/// - Template with no placeholders (valid but warned)
///
/// Both top-level task fields and metadata-backed variables from schedule
/// triggers are accepted as valid.
pub fn validate_template(template: &str) -> Vec<String> {
    let mut warnings = Vec::new();

    if template.trim().is_empty() {
        warnings.push("Template is empty".to_string());
        return warnings;
    }

    let mut found_any = false;
    let mut pos = 0;
    while let Some(start) = template[pos..].find("{{") {
        let abs_start = pos + start;
        if let Some(end) = template[abs_start + 2..].find("}}") {
            let var_name = template[abs_start + 2..abs_start + 2 + end].trim();
            found_any = true;

            if !KNOWN_VARIABLES.contains(&var_name) {
                warnings.push(format!(
                    "Unknown template variable '{{{{{}}}}}'. Known variables: {}",
                    var_name,
                    KNOWN_VARIABLES.join(", ")
                ));
            }

            pos = abs_start + 2 + end + 2;
        } else {
            warnings.push(format!("Unclosed template placeholder at position {}", abs_start));
            break;
        }
    }

    if !found_any {
        warnings.push(
            "Template contains no {{variables}} — the prompt will be the same for every task"
                .to_string(),
        );
    }

    warnings
}

/// Render a prompt template by replacing `{{placeholder}}` tokens with task data.
///
/// Top-level task fields (`title`, `body`, `url`, etc.) are replaced first.
/// Then, any remaining `{{variable}}` placeholders are looked up in
/// `task.metadata`, enabling schedule-based triggers to populate custom
/// variables like `{{fire_time}}` and `{{cron_expression}}`.
pub fn render_template(template: &str, task: &Task) -> String {
    // Phase 1: Replace top-level task fields.
    let result = template
        .replace("{{title}}", &task.title)
        .replace("{{body}}", &task.body)
        .replace("{{url}}", &task.url)
        .replace("{{labels}}", &task.labels.join(", "))
        .replace("{{assignee}}", task.assignee.as_deref().unwrap_or(""))
        .replace("{{source_id}}", &task.source_id)
        .replace(
            "{{metadata}}",
            &task
                .metadata
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n"),
        );

    // Phase 2: Replace metadata-backed variables.
    // Scan for remaining {{...}} placeholders and resolve from task.metadata.
    let mut output = String::with_capacity(result.len());
    let mut pos = 0;

    while pos < result.len() {
        if let Some(start) = result[pos..].find("{{") {
            let abs_start = pos + start;
            // Copy everything before the placeholder.
            output.push_str(&result[pos..abs_start]);

            if let Some(end) = result[abs_start + 2..].find("}}") {
                let var_name = result[abs_start + 2..abs_start + 2 + end].trim();

                // Look up in metadata; if not found, leave the placeholder as-is.
                if let Some(value) = task.metadata.get(var_name) {
                    output.push_str(value);
                } else {
                    // Preserve the original placeholder for unknown variables.
                    output.push_str(&result[abs_start..abs_start + 2 + end + 2]);
                }
                pos = abs_start + 2 + end + 2;
            } else {
                // Unclosed placeholder — copy the rest as-is.
                output.push_str(&result[abs_start..]);
                pos = result.len();
            }
        } else {
            // No more placeholders — copy the rest.
            output.push_str(&result[pos..]);
            break;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_task() -> Task {
        Task {
            source_id: "42".to_string(),
            title: "Fix login bug".to_string(),
            body: "Users can't log in with SSO.".to_string(),
            url: "https://github.com/org/repo/issues/42".to_string(),
            labels: vec!["bug".to_string(), "auth".to_string()],
            assignee: Some("alice".to_string()),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_basic_replacement() {
        let template = "Fix issue #{{source_id}}: {{title}}\n\n{{body}}\n\nURL: {{url}}";
        let result = render_template(template, &sample_task());
        assert!(result.contains("Fix issue #42: Fix login bug"));
        assert!(result.contains("Users can't log in with SSO."));
        assert!(result.contains("https://github.com/org/repo/issues/42"));
    }

    #[test]
    fn test_labels_and_assignee() {
        let template = "Labels: {{labels}}, Assigned to: {{assignee}}";
        let result = render_template(template, &sample_task());
        assert_eq!(result, "Labels: bug, auth, Assigned to: alice");
    }

    #[test]
    fn test_missing_assignee() {
        let mut task = sample_task();
        task.assignee = None;
        let result = render_template("Assignee: {{assignee}}", &task);
        assert_eq!(result, "Assignee: ");
    }

    #[test]
    fn test_metadata() {
        let mut task = sample_task();
        task.metadata.insert("priority".to_string(), "high".to_string());
        let result = render_template("Meta: {{metadata}}", &task);
        assert!(result.contains("priority: high"));
    }

    #[test]
    fn test_no_placeholders() {
        let template = "Do something generic";
        let result = render_template(template, &sample_task());
        assert_eq!(result, "Do something generic");
    }

    #[test]
    fn test_validate_valid_template() {
        let warnings = validate_template("Fix: {{title}}\n\n{{body}}");
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_unknown_variable() {
        let warnings = validate_template("{{title}} {{unknown_var}}");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Unknown template variable"));
        assert!(warnings[0].contains("unknown_var"));
    }

    #[test]
    fn test_validate_empty_template() {
        let warnings = validate_template("");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("empty"));
    }

    #[test]
    fn test_validate_no_variables() {
        let warnings = validate_template("Do something static");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("no {{variables}}"));
    }

    #[test]
    fn test_validate_unclosed_placeholder() {
        let warnings = validate_template("Fix: {{title");
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("Unclosed")));
    }

    #[test]
    fn test_validate_all_known_variables() {
        let template =
            "{{title}} {{body}} {{url}} {{labels}} {{assignee}} {{source_id}} {{metadata}}";
        let warnings = validate_template(template);
        assert!(warnings.is_empty());
    }

    // ── Metadata-backed variable tests ──────────────────────────────

    #[test]
    fn test_render_metadata_variable_fire_time() {
        let mut task = sample_task();
        task.metadata.insert("fire_time".to_string(), "2025-06-01T09:00:00Z".to_string());
        let result = render_template("Fired at: {{fire_time}}", &task);
        assert_eq!(result, "Fired at: 2025-06-01T09:00:00Z");
    }

    #[test]
    fn test_render_metadata_variable_cron_expression() {
        let mut task = sample_task();
        task.metadata.insert("cron_expression".to_string(), "0 9 * * MON-FRI".to_string());
        let result = render_template("Schedule: {{cron_expression}}", &task);
        assert_eq!(result, "Schedule: 0 9 * * MON-FRI");
    }

    #[test]
    fn test_render_multiple_metadata_variables() {
        let mut task = sample_task();
        task.metadata.insert("fire_time".to_string(), "2025-06-01T09:00:00Z".to_string());
        task.metadata.insert("cron_expression".to_string(), "0 9 * * MON-FRI".to_string());
        task.metadata.insert("trigger_type".to_string(), "cron".to_string());
        let result = render_template(
            "Type: {{trigger_type}}, Fired: {{fire_time}}, Expr: {{cron_expression}}",
            &task,
        );
        assert_eq!(result, "Type: cron, Fired: 2025-06-01T09:00:00Z, Expr: 0 9 * * MON-FRI");
    }

    #[test]
    fn test_render_delay_metadata_variables() {
        let mut task = sample_task();
        task.metadata.insert("run_at".to_string(), "2025-07-01T12:00:00Z".to_string());
        task.metadata.insert("workflow_id".to_string(), "abc-123".to_string());
        let result = render_template("Delay: {{run_at}}, Workflow: {{workflow_id}}", &task);
        assert_eq!(result, "Delay: 2025-07-01T12:00:00Z, Workflow: abc-123");
    }

    #[test]
    fn test_render_mixed_top_level_and_metadata() {
        let mut task = sample_task();
        task.metadata.insert("fire_time".to_string(), "2025-06-01T09:00:00Z".to_string());
        let result = render_template("{{title}} fired at {{fire_time}}", &task);
        assert_eq!(result, "Fix login bug fired at 2025-06-01T09:00:00Z");
    }

    #[test]
    fn test_render_missing_metadata_preserves_placeholder() {
        let task = sample_task();
        let result = render_template("Fired: {{fire_time}}", &task);
        // fire_time not in metadata — placeholder should be preserved.
        assert_eq!(result, "Fired: {{fire_time}}");
    }

    #[test]
    fn test_validate_metadata_variables_accepted() {
        let template =
            "{{fire_time}} {{cron_expression}} {{trigger_type}} {{run_at}} {{workflow_id}}";
        let warnings = validate_template(template);
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_all_variables_combined() {
        let template = "{{title}} {{body}} {{url}} {{labels}} {{assignee}} {{source_id}} {{metadata}} {{fire_time}} {{cron_expression}} {{trigger_type}} {{run_at}} {{workflow_id}}";
        let warnings = validate_template(template);
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_validate_still_rejects_truly_unknown() {
        let warnings = validate_template("{{fire_time}} {{totally_fake}}");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("totally_fake"));
    }

    #[test]
    fn test_existing_templates_unchanged() {
        // Ensure the original test still works identically.
        let template = "Fix issue #{{source_id}}: {{title}}\n\n{{body}}\n\nURL: {{url}}";
        let result = render_template(template, &sample_task());
        assert!(result.contains("Fix issue #42: Fix login bug"));
        assert!(result.contains("Users can't log in with SSO."));
        assert!(result.contains("https://github.com/org/repo/issues/42"));
    }

    #[test]
    fn test_render_cron_task_full_template() {
        // Simulate a realistic cron trigger task.
        let mut task = Task {
            source_id: "cron:2025-06-01T09:00:00Z".to_string(),
            title: "Cron trigger: 0 9 * * MON-FRI".to_string(),
            body: String::new(),
            url: String::new(),
            labels: vec![],
            assignee: None,
            metadata: HashMap::new(),
        };
        task.metadata.insert("fire_time".to_string(), "2025-06-01T09:00:00Z".to_string());
        task.metadata.insert("cron_expression".to_string(), "0 9 * * MON-FRI".to_string());

        let template = "Cron job fired at {{fire_time}} (schedule: {{cron_expression}}).\nRun the daily report generation.";
        let result = render_template(template, &task);
        assert_eq!(
            result,
            "Cron job fired at 2025-06-01T09:00:00Z (schedule: 0 9 * * MON-FRI).\nRun the daily report generation."
        );
    }
}
