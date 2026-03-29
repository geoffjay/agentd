use crate::scheduler::types::Task;

/// All known template variables (top-level task fields + metadata-backed).
///
/// Top-level fields are derived directly from `Task` struct fields.
/// Metadata-backed variables are stored in `Task.metadata` and populated
/// by schedule-based triggers. Which metadata variables are present depends
/// on the trigger type:
///
/// | Variable             | Trigger types          | Description                                    |
/// |----------------------|------------------------|------------------------------------------------|
/// | `fire_time`          | cron                   | RFC 3339 timestamp of the firing               |
/// | `cron_expression`    | cron                   | The cron expression that fired                 |
/// | `trigger_type`       | cron, delay            | The trigger type name                          |
/// | `run_at`             | delay                  | The scheduled run-at datetime                  |
/// | `workflow_id`        | delay                  | The workflow UUID                              |
/// | `source_workflow_id` | dispatch_result        | UUID of the workflow whose dispatch completed  |
/// | `dispatch_id`        | dispatch_result        | UUID of the completed dispatch record          |
/// | `status`             | dispatch_result        | Completion status (`completed` or `failed`)    |
/// | `timestamp`          | dispatch_result        | RFC 3339 timestamp of the completion event     |
/// | `original_source_id` | dispatch_result        | Source ID from the parent dispatch (if any)    |
/// | `action`             | webhook (GitHub/Linear)| Event action (e.g., `"opened"`, `"create"`)    |
/// | `github_event`       | webhook (GitHub)       | GitHub event type (e.g., `"issues"`)           |
/// | `delivery_id`        | webhook (GitHub)       | GitHub delivery UUID (`X-GitHub-Delivery`)     |
/// | `issue_number`       | webhook (GitHub)       | GitHub issue number                            |
/// | `pr_number`          | webhook (GitHub)       | GitHub pull request number                     |
/// | `linear_event`       | webhook (Linear)       | Linear event type (e.g., `"Issue"`)            |
/// | `linear_action`      | webhook (Linear)       | Linear action (e.g., `"create"`, `"update"`)   |
/// | `linear_delivery_id` | webhook (Linear)       | Linear delivery ID (`Linear-Delivery` header)  |
/// | `identifier`         | linear_issues, webhook (Linear) | Linear issue identifier (e.g., `"ENG-123"`) |
/// | `state`              | linear_issues, webhook (Linear) | Linear issue state name (e.g., `"Todo"`)    |
/// | `priority`           | linear_issues, webhook (Linear) | Linear priority level (0 = none, 1 = urgent)|
/// | `team`               | linear_issues, webhook (Linear) | Linear team key (e.g., `"ENG"`)             |
/// | `team_name`          | linear_issues, webhook (Linear) | Linear team display name                    |
/// | `project`            | linear_issues          | Linear project name                            |
/// | `linear_id`          | linear_issues, webhook (Linear) | Internal Linear UUID (stable dedup key)     |
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
    // Metadata-backed (dispatch_result triggers)
    "source_workflow_id",
    "dispatch_id",
    "status",
    "timestamp",
    "original_source_id",
    // Metadata-backed (webhook triggers — GitHub)
    // Note: `action` is also used by Linear webhooks as `linear_action`; the
    // GitHub `action` key and the Linear `linear_action` key are kept separate
    // to avoid ambiguity when both header types could be present.
    "action",
    "github_event",
    "delivery_id",
    "issue_number",
    "pr_number",
    // Metadata-backed (webhook triggers — Linear)
    "linear_event",
    "linear_action",
    "linear_delivery_id",
    // Metadata-backed (linear_issues trigger + Linear webhooks)
    "identifier",
    "state",
    "priority",
    "team",
    "team_name",
    "project",
    "linear_id",
    // Metadata-backed (composite triggers)
    "composite_sub_source_ids",
    // Metadata-backed (queue trigger)
    "queue_name",
    "queue_task_id",
    "queue_priority",
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

    // ── dispatch_result trigger variable tests ───────────────────────

    #[test]
    fn test_validate_dispatch_result_variables_accepted() {
        let template = "{{source_workflow_id}} {{dispatch_id}} {{status}} {{timestamp}} {{original_source_id}}";
        let warnings = validate_template(template);
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_render_dispatch_result_variables() {
        let mut task = sample_task();
        task.metadata.insert("source_workflow_id".to_string(), "wf-uuid-123".to_string());
        task.metadata.insert("dispatch_id".to_string(), "dispatch-uuid-456".to_string());
        task.metadata.insert("status".to_string(), "completed".to_string());
        task.metadata.insert("timestamp".to_string(), "2025-06-01T10:00:00Z".to_string());
        task.metadata.insert("original_source_id".to_string(), "42".to_string());

        let template = "Workflow {{source_workflow_id}} dispatch {{dispatch_id}} finished with status {{status}} at {{timestamp}}. Original issue: {{original_source_id}}";
        let result = render_template(template, &task);
        assert_eq!(
            result,
            "Workflow wf-uuid-123 dispatch dispatch-uuid-456 finished with status completed at 2025-06-01T10:00:00Z. Original issue: 42"
        );
    }

    #[test]
    fn test_render_dispatch_result_without_original_source_id() {
        // When original_source_id is absent (source_id was None in the event),
        // the placeholder is preserved as-is.
        let mut task = sample_task();
        task.metadata.insert("source_workflow_id".to_string(), "wf-uuid-123".to_string());
        task.metadata.insert("status".to_string(), "failed".to_string());

        let result = render_template("Status: {{status}}, Origin: {{original_source_id}}", &task);
        assert_eq!(result, "Status: failed, Origin: {{original_source_id}}");
    }

    #[test]
    fn test_validate_all_variables_including_dispatch_result() {
        let template = "{{title}} {{body}} {{url}} {{labels}} {{assignee}} {{source_id}} {{metadata}} \
                        {{fire_time}} {{cron_expression}} {{trigger_type}} {{run_at}} {{workflow_id}} \
                        {{source_workflow_id}} {{dispatch_id}} {{status}} {{timestamp}} {{original_source_id}}";
        let warnings = validate_template(template);
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }

    // ── linear_issues trigger variable tests ────────────────────────

    #[test]
    fn test_validate_linear_variables_accepted() {
        let template =
            "{{identifier}} {{state}} {{priority}} {{team}} {{team_name}} {{project}} {{linear_id}}";
        let warnings = validate_template(template);
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_render_linear_identifier_and_title() {
        let mut task = sample_task();
        task.metadata.insert("identifier".to_string(), "ENG-123".to_string());
        let result = render_template("Work on {{identifier}}: {{title}}", &task);
        assert_eq!(result, "Work on ENG-123: Fix login bug");
    }

    #[test]
    fn test_render_linear_all_metadata_variables() {
        let mut task = sample_task();
        task.metadata.insert("identifier".to_string(), "ENG-42".to_string());
        task.metadata.insert("state".to_string(), "In Progress".to_string());
        task.metadata.insert("priority".to_string(), "1".to_string());
        task.metadata.insert("team".to_string(), "ENG".to_string());
        task.metadata.insert("team_name".to_string(), "Engineering".to_string());
        task.metadata.insert("project".to_string(), "Backend".to_string());
        task.metadata.insert("linear_id".to_string(), "abc-uuid-def".to_string());

        let template = "{{identifier}}: {{title}}\nTeam: {{team}} ({{team_name}}) | Project: {{project}} | Priority: {{priority}} | State: {{state}}";
        let result = render_template(template, &task);
        assert_eq!(
            result,
            "ENG-42: Fix login bug\nTeam: ENG (Engineering) | Project: Backend | Priority: 1 | State: In Progress"
        );
    }

    #[test]
    fn test_render_linear_missing_optional_fields_preserves_placeholder() {
        // state and project are optional — if the Linear issue has no project,
        // the placeholder is preserved in the output.
        let mut task = sample_task();
        task.metadata.insert("identifier".to_string(), "ENG-7".to_string());
        // state and project intentionally absent

        let result =
            render_template("{{identifier}} | State: {{state}} | Project: {{project}}", &task);
        assert_eq!(result, "ENG-7 | State: {{state}} | Project: {{project}}");
    }

    #[test]
    fn test_validate_all_variables_including_linear() {
        let template = "{{title}} {{body}} {{url}} {{labels}} {{assignee}} {{source_id}} {{metadata}} \
                        {{fire_time}} {{cron_expression}} {{trigger_type}} {{run_at}} {{workflow_id}} \
                        {{source_workflow_id}} {{dispatch_id}} {{status}} {{timestamp}} {{original_source_id}} \
                        {{identifier}} {{state}} {{priority}} {{team}} {{team_name}} {{project}} {{linear_id}}";
        let warnings = validate_template(template);
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_render_linear_full_workflow_template() {
        // Simulate a realistic linear_issues trigger task.
        let mut task = Task {
            source_id: "ENG-123".to_string(),
            title: "Fix authentication timeout".to_string(),
            body: "Session tokens expire too quickly under load.".to_string(),
            url: "https://linear.app/eng/issue/ENG-123".to_string(),
            labels: vec!["bug".to_string(), "auth".to_string()],
            assignee: Some("alice".to_string()),
            metadata: HashMap::new(),
        };
        task.metadata.insert("identifier".to_string(), "ENG-123".to_string());
        task.metadata.insert("state".to_string(), "Todo".to_string());
        task.metadata.insert("priority".to_string(), "2".to_string());
        task.metadata.insert("team".to_string(), "ENG".to_string());
        task.metadata.insert("team_name".to_string(), "Engineering".to_string());
        task.metadata.insert("project".to_string(), "Auth Service".to_string());
        task.metadata.insert("linear_id".to_string(), "stable-uuid-789".to_string());

        let template = "Work on Linear issue {{identifier}}: {{title}}\n\n{{body}}\n\nTeam: {{team}} | Project: {{project}} | Priority: {{priority}} | State: {{state}}";
        let result = render_template(template, &task);
        assert_eq!(
            result,
            "Work on Linear issue ENG-123: Fix authentication timeout\n\nSession tokens expire too quickly under load.\n\nTeam: ENG | Project: Auth Service | Priority: 2 | State: Todo"
        );
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
