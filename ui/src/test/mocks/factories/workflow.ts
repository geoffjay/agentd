/**
 * Workflow factory — builds Workflow objects for tests.
 *
 * `makeTrigger` provides a representative config per trigger type so
 * tests can cover the full TriggerConfig union without hand-writing
 * payloads.
 */

import type {
	TriggerConfig,
	TriggerType,
	Workflow,
} from "@/types/orchestrator";

let counter = 0;

/** Representative TriggerConfig for each type. */
export function makeTrigger(type: TriggerType): TriggerConfig {
	switch (type) {
		case "github_issues":
			return {
				type,
				owner: "geoffjay",
				repo: "agentd",
				labels: ["bug"],
				state: "open",
			};
		case "github_pull_requests":
			return {
				type,
				owner: "geoffjay",
				repo: "agentd",
				labels: [],
				state: "open",
			};
		case "cron":
			return { type, expression: "0 9 * * MON-FRI" };
		case "delay":
			return { type, run_at: "2026-07-01T12:00:00Z" };
		case "agent_lifecycle":
			return { type, event: "session_start" };
		case "dispatch_result":
			return { type, status: "completed" };
		case "webhook":
			return { type, secret: "shh", source: "git_hub" };
		case "manual":
			return { type };
		case "agent_idle":
			return { type, idle_seconds: 120 };
		case "linear_issues":
			return { type, team_key: "ENG", labels: ["bug"] };
		case "composite":
			return {
				type,
				mode: "and",
				triggers: [makeTrigger("manual"), makeTrigger("cron")],
				correlation_window_secs: 90,
			};
		case "queue":
			return { type, queue_name: "review-queue", poll_interval_secs: 5 };
		case "ask_response":
			return { type, category: "health" };
		case "gitlab_issues":
			return {
				type,
				owner: "mygroup",
				repo: "myproject",
				labels: [],
				state: "opened",
			};
		case "gitlab_merge_requests":
			return {
				type,
				owner: "mygroup",
				repo: "myproject",
				labels: ["needs-review"],
				state: "opened",
			};
	}
}

export function makeWorkflow(overrides: Partial<Workflow> = {}): Workflow {
	counter += 1;
	return {
		id: `00000000-0000-4000-8000-${String(counter).padStart(12, "0")}`,
		name: `workflow-${counter}`,
		agent_id: "11111111-0000-4000-8000-000000000001",
		trigger_config: makeTrigger("github_issues"),
		prompt_template: "Work on {{title}}: {{body}}",
		poll_interval_secs: 900,
		enabled: true,
		tool_policy: { mode: "allow_all" },
		created_at: "2026-01-01T00:00:00Z",
		updated_at: "2026-01-01T00:00:00Z",
		...overrides,
	};
}
