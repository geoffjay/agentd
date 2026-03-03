use crate::scheduler::types::Task;

/// Known template variables that will be replaced during rendering.
pub const KNOWN_VARIABLES: &[&str] =
    &["title", "body", "url", "labels", "assignee", "source_id", "metadata"];

/// Validate a prompt template, returning any warnings or errors.
///
/// Checks for:
/// - Unknown `{{variable}}` placeholders that won't be replaced
/// - Empty template
/// - Template with no placeholders (valid but warned)
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
pub fn render_template(template: &str, task: &Task) -> String {
    template
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
        )
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
}
