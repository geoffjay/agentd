use crate::scheduler::types::Task;

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
}
