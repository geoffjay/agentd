/**
 * Trigger registry — one definition per TriggerConfig variant.
 *
 * Each definition declares the form fields a trigger needs, the prompt
 * template variables it provides (mirroring KNOWN_VARIABLES in
 * crates/orchestrator/src/scheduler/template.rs), whether it polls, and
 * the conversions between flat string form values and the typed
 * TriggerConfig sent to the API.
 *
 * The composite trigger is the one exception: it has no flat fields and
 * is edited by the recursive CompositeTriggerEditor instead.
 */

import type { TriggerConfig, TriggerType } from "@/types/orchestrator";
import { TRIGGER_TYPE_LABELS } from "@/utils/triggers";

// ---------------------------------------------------------------------------
// Field + variable definitions
// ---------------------------------------------------------------------------

export type TriggerFieldInput =
	| "text"
	| "number"
	| "select"
	| "csv"
	| "string-list"
	| "datetime"
	| "secret"
	| "regex";

export interface TriggerFieldDef {
	key: string;
	label: string;
	input: TriggerFieldInput;
	required?: boolean;
	options?: Array<{ label: string; value: string }>;
	placeholder?: string;
	help?: string;
	defaultValue?: string | string[];
}

export interface TriggerVariableDef {
	name: string;
	description: string;
	sample: string;
}

/** Flat form values for one trigger: strings, or string arrays for lists. */
export type TriggerFieldValues = Record<string, string | string[]>;

export type TriggerGroup = "Polling" | "Schedule" | "Events" | "Advanced";

export interface TriggerDef {
	type: TriggerType;
	label: string;
	group: TriggerGroup;
	description: string;
	/** Whether the trigger polls and therefore uses the poll interval. */
	polls: boolean;
	fields: TriggerFieldDef[];
	/** Trigger-specific prompt variables (in addition to BASE_VARIABLES). */
	variables: TriggerVariableDef[];
	toConfig(values: TriggerFieldValues): TriggerConfig;
	fromConfig(config: TriggerConfig): TriggerFieldValues;
}

/** Task variables available to every trigger type. */
export const BASE_VARIABLES: TriggerVariableDef[] = [
	{ name: "title", description: "Task title", sample: "Fix login bug" },
	{
		name: "body",
		description: "Task body / description",
		sample: "Users cannot log in with SSO...",
	},
	{
		name: "url",
		description: "Source URL",
		sample: "https://github.com/owner/repo/issues/42",
	},
	{
		name: "labels",
		description: "Comma-separated labels",
		sample: "bug, high-priority",
	},
	{ name: "assignee", description: "Task assignee", sample: "geoffjay" },
	{
		name: "source_id",
		description: "Source identifier (e.g. issue number)",
		sample: "42",
	},
	{
		name: "metadata",
		description: "All metadata as key: value lines",
		sample: "key: value",
	},
];

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

function str(values: TriggerFieldValues, key: string): string {
	const v = values[key];
	return typeof v === "string" ? v.trim() : "";
}

function list(values: TriggerFieldValues, key: string): string[] {
	const v = values[key];
	if (Array.isArray(v)) return v.map((s) => s.trim()).filter(Boolean);
	if (typeof v === "string")
		return v
			.split(",")
			.map((s) => s.trim())
			.filter(Boolean);
	return [];
}

function optional(value: string): string | undefined {
	return value === "" ? undefined : value;
}

function optionalNumber(value: string): number | undefined {
	const n = Number.parseInt(value, 10);
	return Number.isNaN(n) ? undefined : n;
}

const csvOf = (labels?: string[]) => (labels ?? []).join(", ");

/** Typed empty values — keeps fromConfig branches assignable to the index signature. */
const NO_VALUES: TriggerFieldValues = {};

// ---------------------------------------------------------------------------
// Shared field fragments
// ---------------------------------------------------------------------------

function repoFields(host: "GitHub" | "GitLab"): TriggerFieldDef[] {
	return [
		{
			key: "owner",
			label: host === "GitHub" ? "Owner" : "Namespace",
			input: "text",
			required: true,
			placeholder: host === "GitHub" ? "e.g. geoffjay" : "e.g. mygroup",
		},
		{
			key: "repo",
			label: host === "GitHub" ? "Repository" : "Project",
			input: "text",
			required: true,
			placeholder: "e.g. agentd",
		},
		{
			key: "labels",
			label: "Labels",
			input: "csv",
			placeholder: "e.g. bug, enhancement",
			help: "Comma-separated; tasks must carry these labels.",
		},
	];
}

const ASSIGNEE_FIELD: TriggerFieldDef = {
	key: "assignee",
	label: "Assignee",
	input: "text",
	placeholder: "username",
};

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

export const TRIGGER_DEFS: TriggerDef[] = [
	{
		type: "github_issues",
		label: TRIGGER_TYPE_LABELS.github_issues,
		group: "Polling",
		description: "Poll a GitHub repository for matching issues.",
		polls: true,
		fields: [
			...repoFields("GitHub"),
			{
				key: "state",
				label: "Issue state",
				input: "select",
				defaultValue: "open",
				options: [
					{ label: "Open", value: "open" },
					{ label: "Closed", value: "closed" },
					{ label: "All", value: "all" },
				],
			},
			ASSIGNEE_FIELD,
		],
		variables: [],
		toConfig: (v) => ({
			type: "github_issues",
			owner: str(v, "owner"),
			repo: str(v, "repo"),
			labels: list(v, "labels"),
			state: (str(v, "state") || "open") as "open" | "closed" | "all",
			assignee: optional(str(v, "assignee")),
		}),
		fromConfig: (c) =>
			c.type === "github_issues"
				? {
						owner: c.owner,
						repo: c.repo,
						labels: csvOf(c.labels),
						state: c.state ?? "open",
						assignee: c.assignee ?? "",
					}
				: NO_VALUES,
	},
	{
		type: "github_pull_requests",
		label: TRIGGER_TYPE_LABELS.github_pull_requests,
		group: "Polling",
		description: "Poll a GitHub repository for matching pull requests.",
		polls: true,
		fields: [
			...repoFields("GitHub"),
			{
				key: "state",
				label: "PR state",
				input: "select",
				defaultValue: "open",
				options: [
					{ label: "Open", value: "open" },
					{ label: "Closed", value: "closed" },
					{ label: "Merged", value: "merged" },
					{ label: "All", value: "all" },
				],
			},
			ASSIGNEE_FIELD,
		],
		variables: [
			{ name: "head_ref", description: "PR head branch", sample: "feature/x" },
			{ name: "base_ref", description: "PR base branch", sample: "main" },
			{
				name: "is_draft",
				description: "Whether the PR is a draft",
				sample: "false",
			},
		],
		toConfig: (v) => ({
			type: "github_pull_requests",
			owner: str(v, "owner"),
			repo: str(v, "repo"),
			labels: list(v, "labels"),
			state: (str(v, "state") || "open") as
				| "open"
				| "closed"
				| "merged"
				| "all",
			assignee: optional(str(v, "assignee")),
		}),
		fromConfig: (c) =>
			c.type === "github_pull_requests"
				? {
						owner: c.owner,
						repo: c.repo,
						labels: csvOf(c.labels),
						state: c.state ?? "open",
						assignee: c.assignee ?? "",
					}
				: NO_VALUES,
	},
	{
		type: "cron",
		label: TRIGGER_TYPE_LABELS.cron,
		group: "Schedule",
		description: "Fire on a recurring cron schedule.",
		polls: false,
		fields: [
			{
				key: "expression",
				label: "Cron expression",
				input: "text",
				required: true,
				placeholder: "0 9 * * MON-FRI",
				help: 'Standard cron syntax, e.g. "0 9 * * MON-FRI" for weekdays at 09:00.',
			},
		],
		variables: [
			{
				name: "fire_time",
				description: "When the schedule fired",
				sample: "2026-06-11T09:00:00Z",
			},
			{
				name: "cron_expression",
				description: "The cron expression",
				sample: "0 9 * * MON-FRI",
			},
			{
				name: "trigger_type",
				description: "Trigger type name",
				sample: "cron",
			},
			{
				name: "workflow_id",
				description: "This workflow's UUID",
				sample: "6f1f...",
			},
		],
		toConfig: (v) => ({ type: "cron", expression: str(v, "expression") }),
		fromConfig: (c) =>
			c.type === "cron" ? { expression: c.expression } : NO_VALUES,
	},
	{
		type: "delay",
		label: TRIGGER_TYPE_LABELS.delay,
		group: "Schedule",
		description: "Fire once at a specific date and time, then auto-disable.",
		polls: false,
		fields: [
			{
				key: "run_at",
				label: "Run at",
				input: "datetime",
				required: true,
				help: "The workflow fires once at this time and is then disabled.",
			},
		],
		variables: [
			{
				name: "run_at",
				description: "Scheduled fire time",
				sample: "2026-07-01T12:00:00Z",
			},
			{
				name: "fire_time",
				description: "When the trigger fired",
				sample: "2026-07-01T12:00:00Z",
			},
			{
				name: "trigger_type",
				description: "Trigger type name",
				sample: "delay",
			},
			{
				name: "workflow_id",
				description: "This workflow's UUID",
				sample: "6f1f...",
			},
		],
		toConfig: (v) => ({ type: "delay", run_at: str(v, "run_at") }),
		fromConfig: (c) => (c.type === "delay" ? { run_at: c.run_at } : NO_VALUES),
	},
	{
		type: "agent_lifecycle",
		label: TRIGGER_TYPE_LABELS.agent_lifecycle,
		group: "Events",
		description: "Fire when an agent lifecycle event occurs.",
		polls: false,
		fields: [
			{
				key: "event",
				label: "Lifecycle event",
				input: "select",
				required: true,
				defaultValue: "session_start",
				options: [
					{ label: "Session start", value: "session_start" },
					{ label: "Session end", value: "session_end" },
					{ label: "Context clear", value: "context_clear" },
				],
			},
		],
		variables: [
			{
				name: "event_type",
				description: "The lifecycle event",
				sample: "session_start",
			},
			{
				name: "agent_id",
				description: "Agent the event concerns",
				sample: "6f1f...",
			},
			{
				name: "timestamp",
				description: "When the event occurred",
				sample: "2026-06-11T09:00:00Z",
			},
		],
		toConfig: (v) => ({
			type: "agent_lifecycle",
			event: (str(v, "event") || "session_start") as
				| "session_start"
				| "session_end"
				| "context_clear",
		}),
		fromConfig: (c) =>
			c.type === "agent_lifecycle" ? { event: c.event } : NO_VALUES,
	},
	{
		type: "dispatch_result",
		label: TRIGGER_TYPE_LABELS.dispatch_result,
		group: "Events",
		description:
			"Fire when another workflow's dispatch completes — enables chaining.",
		polls: false,
		fields: [
			{
				key: "source_workflow_id",
				label: "Source workflow ID",
				input: "text",
				placeholder: "UUID — leave empty for any workflow",
			},
			{
				key: "status",
				label: "Completion status",
				input: "select",
				defaultValue: "",
				options: [
					{ label: "Any", value: "" },
					{ label: "Completed", value: "completed" },
					{ label: "Failed", value: "failed" },
					{ label: "Skipped", value: "skipped" },
				],
			},
		],
		variables: [
			{
				name: "source_workflow_id",
				description: "Workflow that completed",
				sample: "6f1f...",
			},
			{
				name: "dispatch_id",
				description: "The completed dispatch",
				sample: "9a2b...",
			},
			{ name: "status", description: "Dispatch status", sample: "completed" },
			{
				name: "timestamp",
				description: "Completion time",
				sample: "2026-06-11T09:00:00Z",
			},
			{
				name: "original_source_id",
				description: "Source ID of the original task",
				sample: "42",
			},
		],
		toConfig: (v) => ({
			type: "dispatch_result",
			source_workflow_id: optional(str(v, "source_workflow_id")),
			status: optional(str(v, "status")) as
				| "completed"
				| "failed"
				| "skipped"
				| undefined,
		}),
		fromConfig: (c) =>
			c.type === "dispatch_result"
				? {
						source_workflow_id: c.source_workflow_id ?? "",
						status: c.status ?? "",
					}
				: NO_VALUES,
	},
	{
		type: "webhook",
		label: TRIGGER_TYPE_LABELS.webhook,
		group: "Events",
		description:
			"Fire on incoming webhooks (POST /webhooks/{workflow_id}). Supports GitHub and Linear payloads.",
		polls: false,
		fields: [
			{
				key: "secret",
				label: "HMAC secret",
				input: "secret",
				placeholder: "Shared secret for signature verification",
				help: "Optional. When set, payload signatures are verified.",
			},
			{
				key: "source",
				label: "Expected source",
				input: "select",
				defaultValue: "any",
				options: [
					{ label: "Any", value: "any" },
					{ label: "GitHub", value: "git_hub" },
					{ label: "Linear", value: "linear" },
				],
				help: "Enforced by the handler to prevent source spoofing.",
			},
		],
		variables: [
			{ name: "action", description: "GitHub event action", sample: "opened" },
			{
				name: "github_event",
				description: "GitHub event name",
				sample: "issues",
			},
			{
				name: "delivery_id",
				description: "GitHub delivery ID",
				sample: "72d3...",
			},
			{
				name: "issue_number",
				description: "Issue number (issue events)",
				sample: "42",
			},
			{ name: "pr_number", description: "PR number (PR events)", sample: "7" },
			{
				name: "linear_event",
				description: "Linear event name",
				sample: "Issue",
			},
			{
				name: "linear_action",
				description: "Linear event action",
				sample: "create",
			},
			{
				name: "linear_delivery_id",
				description: "Linear delivery ID",
				sample: "8c1d...",
			},
			{
				name: "timestamp",
				description: "Receipt time",
				sample: "2026-06-11T09:00:00Z",
			},
		],
		toConfig: (v) => ({
			type: "webhook",
			secret: optional(str(v, "secret")),
			source: (str(v, "source") || "any") as "git_hub" | "linear" | "any",
		}),
		fromConfig: (c) =>
			c.type === "webhook"
				? { secret: c.secret ?? "", source: c.source ?? "any" }
				: NO_VALUES,
	},
	{
		type: "manual",
		label: TRIGGER_TYPE_LABELS.manual,
		group: "Events",
		description:
			"No automatic trigger — dispatched explicitly via the API or the Trigger button.",
		polls: false,
		fields: [],
		variables: [],
		toConfig: () => ({ type: "manual" }),
		fromConfig: () => ({}),
	},
	{
		type: "agent_idle",
		label: TRIGGER_TYPE_LABELS.agent_idle,
		group: "Events",
		description:
			"Fire when the workflow's agent has been idle for a given number of seconds.",
		polls: false,
		fields: [
			{
				key: "idle_seconds",
				label: "Idle seconds",
				input: "number",
				required: true,
				defaultValue: "60",
				help: "Seconds of inactivity before the workflow fires.",
			},
		],
		variables: [
			{ name: "agent_id", description: "The idle agent", sample: "6f1f..." },
			{
				name: "timestamp",
				description: "When the idle threshold was hit",
				sample: "2026-06-11T09:00:00Z",
			},
		],
		toConfig: (v) => ({
			type: "agent_idle",
			idle_seconds: optionalNumber(str(v, "idle_seconds")) ?? 0,
		}),
		fromConfig: (c) =>
			c.type === "agent_idle"
				? { idle_seconds: String(c.idle_seconds) }
				: NO_VALUES,
	},
	{
		type: "linear_issues",
		label: TRIGGER_TYPE_LABELS.linear_issues,
		group: "Polling",
		description:
			"Poll Linear for matching issues. Requires AGENTD_LINEAR_API_KEY on the orchestrator. At least one filter is required.",
		polls: true,
		fields: [
			{
				key: "team_key",
				label: "Team key",
				input: "text",
				placeholder: "e.g. ENG",
			},
			{
				key: "project",
				label: "Project",
				input: "text",
				placeholder: "name or ID",
			},
			{
				key: "status",
				label: "Statuses",
				input: "csv",
				placeholder: "e.g. Todo, In Progress",
				help: "Comma-separated status names.",
			},
			{
				key: "labels",
				label: "Labels",
				input: "csv",
				placeholder: "e.g. bug",
				help: "Issue must carry all listed labels.",
			},
			ASSIGNEE_FIELD,
		],
		variables: [
			{
				name: "identifier",
				description: "Issue identifier",
				sample: "ENG-123",
			},
			{ name: "state", description: "Issue state", sample: "Todo" },
			{ name: "priority", description: "Issue priority", sample: "2" },
			{ name: "team", description: "Team key", sample: "ENG" },
			{ name: "team_name", description: "Team name", sample: "Engineering" },
			{ name: "project", description: "Project name", sample: "Backend" },
			{
				name: "linear_id",
				description: "Linear issue UUID",
				sample: "8c1d...",
			},
		],
		toConfig: (v) => {
			const status = list(v, "status");
			return {
				type: "linear_issues",
				team_key: optional(str(v, "team_key")),
				project: optional(str(v, "project")),
				status: status.length > 0 ? status : undefined,
				labels: list(v, "labels"),
				assignee: optional(str(v, "assignee")),
			};
		},
		fromConfig: (c) =>
			c.type === "linear_issues"
				? {
						team_key: c.team_key ?? "",
						project: c.project ?? "",
						status: csvOf(c.status ?? undefined),
						labels: csvOf(c.labels),
						assignee: c.assignee ?? "",
					}
				: NO_VALUES,
	},
	{
		type: "composite",
		label: TRIGGER_TYPE_LABELS.composite,
		group: "Advanced",
		description:
			"Combine two or more sub-triggers with AND/OR logic (max 3 nesting levels).",
		polls: true,
		// Edited by CompositeTriggerEditor; no flat fields.
		fields: [],
		variables: [
			{
				name: "composite_sub_source_ids",
				description: "Source IDs of correlated sub-trigger tasks",
				sample: "42, cron:170...",
			},
		],
		toConfig: () => ({ type: "composite", mode: "or", triggers: [] }),
		fromConfig: () => ({}),
	},
	{
		type: "queue",
		label: TRIGGER_TYPE_LABELS.queue,
		group: "Advanced",
		description:
			"Consume tasks from a named internal queue (push via POST /queues/{name}/push).",
		polls: false,
		fields: [
			{
				key: "queue_name",
				label: "Queue name",
				input: "text",
				required: true,
				placeholder: "e.g. review-queue",
				help: "Alphanumeric and hyphens, max 64 characters.",
			},
			{
				key: "poll_interval_secs",
				label: "Poll interval (seconds)",
				input: "number",
				placeholder: "5",
				help: "How often to poll when the queue is empty.",
			},
			{
				key: "visibility_timeout_secs",
				label: "Visibility timeout (seconds)",
				input: "number",
				placeholder: "300",
				help: "How long a dequeued task stays invisible before retry.",
			},
		],
		variables: [
			{
				name: "queue_name",
				description: "The queue name",
				sample: "review-queue",
			},
			{
				name: "queue_task_id",
				description: "Queue task UUID",
				sample: "9a2b...",
			},
			{ name: "queue_priority", description: "Task priority", sample: "0" },
		],
		toConfig: (v) => ({
			type: "queue",
			queue_name: str(v, "queue_name"),
			poll_interval_secs: optionalNumber(str(v, "poll_interval_secs")),
			visibility_timeout_secs: optionalNumber(
				str(v, "visibility_timeout_secs"),
			),
		}),
		fromConfig: (c) =>
			c.type === "queue"
				? {
						queue_name: c.queue_name,
						poll_interval_secs:
							c.poll_interval_secs != null ? String(c.poll_interval_secs) : "",
						visibility_timeout_secs:
							c.visibility_timeout_secs != null
								? String(c.visibility_timeout_secs)
								: "",
					}
				: NO_VALUES,
	},
	{
		type: "ask_response",
		label: TRIGGER_TYPE_LABELS.ask_response,
		group: "Events",
		description:
			"Fire when a human answers or dismisses a question asked via the ask service.",
		polls: false,
		fields: [
			{
				key: "agent_id",
				label: "Asking agent",
				input: "text",
				placeholder: "agent name — leave empty for any",
			},
			{
				key: "category",
				label: "Category",
				input: "text",
				placeholder: "e.g. health",
			},
			{
				key: "response_pattern",
				label: "Answer pattern",
				input: "regex",
				placeholder: ".*",
				help: "Regex applied to the answer text.",
			},
		],
		variables: [
			{
				name: "question_id",
				description: "UUID of the answered question",
				sample: "9a2b...",
			},
			{
				name: "agent_id",
				description: "Agent that asked",
				sample: "dietician",
			},
			{ name: "category", description: "Question category", sample: "health" },
			{
				name: "question",
				description: "The question text",
				sample: "Proceed with deploy?",
			},
			{ name: "answer", description: "The human's answer", sample: "yes" },
			{
				name: "event_type",
				description: "question_answered or question_dismissed",
				sample: "question_answered",
			},
			{
				name: "workflow_id",
				description: "Originating workflow",
				sample: "6f1f...",
			},
		],
		toConfig: (v) => ({
			type: "ask_response",
			agent_id: optional(str(v, "agent_id")),
			category: optional(str(v, "category")),
			response_pattern: optional(str(v, "response_pattern")),
		}),
		fromConfig: (c) =>
			c.type === "ask_response"
				? {
						agent_id: c.agent_id ?? "",
						category: c.category ?? "",
						response_pattern: c.response_pattern ?? "",
					}
				: NO_VALUES,
	},
	{
		type: "gitlab_issues",
		label: TRIGGER_TYPE_LABELS.gitlab_issues,
		group: "Polling",
		description:
			"Poll GitLab for matching issues. Requires AGENTD_GITLAB_TOKEN on the orchestrator.",
		polls: true,
		fields: [
			...repoFields("GitLab"),
			{
				key: "state",
				label: "Issue state",
				input: "select",
				defaultValue: "opened",
				options: [
					{ label: "Opened", value: "opened" },
					{ label: "Closed", value: "closed" },
					{ label: "All", value: "all" },
				],
			},
			ASSIGNEE_FIELD,
		],
		variables: [
			{
				name: "gitlab_project_id",
				description: "GitLab project ID",
				sample: "1234",
			},
			{
				name: "gitlab_iid",
				description: "Issue IID within the project",
				sample: "42",
			},
			{ name: "state", description: "Issue state", sample: "opened" },
		],
		toConfig: (v) => ({
			type: "gitlab_issues",
			owner: str(v, "owner"),
			repo: str(v, "repo"),
			labels: list(v, "labels"),
			state: (str(v, "state") || "opened") as "opened" | "closed" | "all",
			assignee: optional(str(v, "assignee")),
		}),
		fromConfig: (c) =>
			c.type === "gitlab_issues"
				? {
						owner: c.owner,
						repo: c.repo,
						labels: csvOf(c.labels),
						state: c.state ?? "opened",
						assignee: c.assignee ?? "",
					}
				: NO_VALUES,
	},
	{
		type: "gitlab_merge_requests",
		label: TRIGGER_TYPE_LABELS.gitlab_merge_requests,
		group: "Polling",
		description:
			"Poll GitLab for matching merge requests. Requires AGENTD_GITLAB_TOKEN on the orchestrator.",
		polls: true,
		fields: [
			...repoFields("GitLab"),
			{
				key: "state",
				label: "MR state",
				input: "select",
				defaultValue: "opened",
				options: [
					{ label: "Opened", value: "opened" },
					{ label: "Closed", value: "closed" },
					{ label: "Merged", value: "merged" },
					{ label: "All", value: "all" },
				],
			},
			ASSIGNEE_FIELD,
		],
		variables: [
			{
				name: "gitlab_project_id",
				description: "GitLab project ID",
				sample: "1234",
			},
			{
				name: "gitlab_iid",
				description: "MR IID within the project",
				sample: "7",
			},
			{ name: "state", description: "MR state", sample: "opened" },
			{
				name: "source_branch",
				description: "MR source branch",
				sample: "feature/x",
			},
			{
				name: "target_branch",
				description: "MR target branch",
				sample: "main",
			},
			{
				name: "merge_status",
				description: "Merge status",
				sample: "can_be_merged",
			},
			{
				name: "draft",
				description: "Whether the MR is a draft",
				sample: "false",
			},
		],
		toConfig: (v) => ({
			type: "gitlab_merge_requests",
			owner: str(v, "owner"),
			repo: str(v, "repo"),
			labels: list(v, "labels"),
			state: (str(v, "state") || "opened") as
				| "opened"
				| "closed"
				| "merged"
				| "all",
			assignee: optional(str(v, "assignee")),
		}),
		fromConfig: (c) =>
			c.type === "gitlab_merge_requests"
				? {
						owner: c.owner,
						repo: c.repo,
						labels: csvOf(c.labels),
						state: c.state ?? "opened",
						assignee: c.assignee ?? "",
					}
				: NO_VALUES,
	},
];

const DEF_BY_TYPE = new Map(TRIGGER_DEFS.map((d) => [d.type, d]));

/** Look up a trigger definition by type. */
export function triggerDef(type: TriggerType): TriggerDef {
	const def = DEF_BY_TYPE.get(type);
	if (!def) throw new Error(`Unknown trigger type: ${type}`);
	return def;
}

/** Default flat values for a trigger type, from its field defaults. */
export function defaultValues(type: TriggerType): TriggerFieldValues {
	const values: TriggerFieldValues = {};
	for (const field of triggerDef(type).fields) {
		values[field.key] = field.defaultValue ?? "";
	}
	return values;
}

/**
 * Prompt variables available for a trigger config: the base task variables
 * plus the trigger's own. Composite aggregates its sub-triggers' variables.
 */
export function variablesFor(config: TriggerConfig): TriggerVariableDef[] {
	const def = triggerDef(config.type);
	if (config.type !== "composite") return [...BASE_VARIABLES, ...def.variables];

	const seen = new Set<string>(BASE_VARIABLES.map((v) => v.name));
	const merged = [...BASE_VARIABLES, ...def.variables];
	for (const v of def.variables) seen.add(v.name);
	for (const sub of config.triggers) {
		for (const v of variablesFor(sub)) {
			if (!seen.has(v.name)) {
				seen.add(v.name);
				merged.push(v);
			}
		}
	}
	return merged;
}
