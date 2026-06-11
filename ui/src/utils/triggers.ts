/**
 * Display helpers for workflow trigger configurations.
 *
 * Covers every TriggerConfig variant so tables and detail pages render a
 * human-readable label and summary without per-page switch statements.
 */

import type { TriggerConfig, TriggerType } from "@/types/orchestrator";

/** Human-readable label for each trigger type. */
export const TRIGGER_TYPE_LABELS: Record<TriggerType, string> = {
	github_issues: "GitHub Issues",
	github_pull_requests: "GitHub Pull Requests",
	cron: "Cron Schedule",
	delay: "One-shot Delay",
	agent_lifecycle: "Agent Lifecycle",
	dispatch_result: "Dispatch Result",
	webhook: "Webhook",
	manual: "Manual",
	agent_idle: "Agent Idle",
	linear_issues: "Linear Issues",
	composite: "Composite",
	queue: "Queue",
	ask_response: "Ask Response",
	gitlab_issues: "GitLab Issues",
	gitlab_merge_requests: "GitLab Merge Requests",
};

/** Label for a trigger type, falling back to the raw type string. */
export function triggerTypeLabel(type: string): string {
	return TRIGGER_TYPE_LABELS[type as TriggerType] ?? type;
}

/**
 * One-line summary of a trigger config, e.g.
 * `GitHub Issues · geoffjay/agentd · Labels: bug` or `Cron · 0 9 * * MON-FRI`.
 */
export function triggerSummary(config: TriggerConfig | undefined): string {
	if (!config) return "No trigger configured";
	const label = triggerTypeLabel(config.type);
	const detail = triggerDetail(config);
	return detail ? `${label} · ${detail}` : label;
}

/** Detail portion of the summary, without the type label. */
export function triggerDetail(config: TriggerConfig): string {
	switch (config.type) {
		case "github_issues":
		case "github_pull_requests":
		case "gitlab_issues":
		case "gitlab_merge_requests": {
			const parts = [`${config.owner}/${config.repo}`];
			if (config.labels && config.labels.length > 0)
				parts.push(`Labels: ${config.labels.join(", ")}`);
			if (config.state) parts.push(`State: ${config.state}`);
			if (config.assignee) parts.push(`Assignee: ${config.assignee}`);
			return parts.join(" · ");
		}
		case "cron":
			return config.expression;
		case "delay":
			return config.run_at;
		case "agent_lifecycle":
			return config.event;
		case "dispatch_result": {
			const parts: string[] = [];
			if (config.source_workflow_id)
				parts.push(`Workflow: ${config.source_workflow_id}`);
			if (config.status) parts.push(`Status: ${config.status}`);
			return parts.join(" · ");
		}
		case "webhook": {
			const parts: string[] = [];
			if (config.source && config.source !== "any")
				parts.push(
					`Source: ${config.source === "git_hub" ? "GitHub" : "Linear"}`,
				);
			if (config.secret) parts.push("HMAC verified");
			return parts.join(" · ");
		}
		case "manual":
			return "";
		case "agent_idle":
			return `Idle ${config.idle_seconds}s`;
		case "linear_issues": {
			const parts: string[] = [];
			if (config.team_key) parts.push(`Team: ${config.team_key}`);
			if (config.project) parts.push(`Project: ${config.project}`);
			if (config.status && config.status.length > 0)
				parts.push(`Status: ${config.status.join(", ")}`);
			if (config.labels && config.labels.length > 0)
				parts.push(`Labels: ${config.labels.join(", ")}`);
			if (config.assignee) parts.push(`Assignee: ${config.assignee}`);
			return parts.join(" · ");
		}
		case "composite":
			return `${config.mode.toUpperCase()} of ${config.triggers.length} triggers`;
		case "queue":
			return config.queue_name;
		case "ask_response": {
			const parts: string[] = [];
			if (config.agent_id) parts.push(`Agent: ${config.agent_id}`);
			if (config.category) parts.push(`Category: ${config.category}`);
			return parts.join(" · ");
		}
		default:
			return "";
	}
}
