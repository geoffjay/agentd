/**
 * Trigger draft + registry tests.
 *
 * Round-trips every trigger type through draftFromConfig → draftToConfig,
 * and exercises the validation rules that mirror the orchestrator.
 */

import { describe, expect, it } from "vitest";
import {
	draftFromConfig,
	draftToConfig,
	MAX_COMPOSITE_DEPTH,
	newTriggerDraft,
	type TriggerDraft,
	validateTriggerDraft,
} from "@/components/workflows/form/triggerDraft";
import {
	BASE_VARIABLES,
	TRIGGER_DEFS,
	variablesFor,
} from "@/components/workflows/form/triggerDefs";
import { makeTrigger } from "@/test/mocks/factories";
import type { TriggerType } from "@/types/orchestrator";

const ALL_TYPES = TRIGGER_DEFS.map((d) => d.type);

describe("triggerDefs registry", () => {
	it("covers all 15 trigger types", () => {
		expect(ALL_TYPES).toHaveLength(15);
	});

	it.each(ALL_TYPES)("%s round-trips config → draft → config", (type) => {
		const config = makeTrigger(type);
		const roundTripped = draftToConfig(draftFromConfig(config));
		expect(roundTripped).toEqual(config);
	});

	it.each(ALL_TYPES)("%s provides the base variables", (type) => {
		const variables = variablesFor(makeTrigger(type));
		for (const base of BASE_VARIABLES) {
			expect(variables.map((v) => v.name)).toContain(base.name);
		}
	});

	it("composite aggregates sub-trigger variables", () => {
		const variables = variablesFor({
			type: "composite",
			mode: "or",
			triggers: [makeTrigger("cron"), makeTrigger("github_pull_requests")],
		});
		const names = variables.map((v) => v.name);
		expect(names).toContain("composite_sub_source_ids");
		expect(names).toContain("cron_expression");
		expect(names).toContain("head_ref");
	});
});

describe("validateTriggerDraft", () => {
	it("requires owner and repo for github triggers", () => {
		const draft = newTriggerDraft("github_issues");
		expect(validateTriggerDraft(draft).join(" ")).toMatch(/Owner is required/);

		draft.values.owner = "geoffjay";
		draft.values.repo = "agentd";
		expect(validateTriggerDraft(draft)).toEqual([]);
	});

	it("requires a positive idle_seconds for agent_idle", () => {
		const draft = newTriggerDraft("agent_idle");
		draft.values.idle_seconds = "0";
		expect(validateTriggerDraft(draft).join(" ")).toMatch(/positive integer/);
	});

	it("requires at least one filter for linear_issues", () => {
		const draft = newTriggerDraft("linear_issues");
		expect(validateTriggerDraft(draft).join(" ")).toMatch(
			/at least one filter/,
		);

		draft.values.team_key = "ENG";
		expect(validateTriggerDraft(draft)).toEqual([]);
	});

	it("rejects invalid queue names", () => {
		const draft = newTriggerDraft("queue");
		draft.values.queue_name = "bad name?";
		expect(validateTriggerDraft(draft).join(" ")).toMatch(/alphanumeric/);

		draft.values.queue_name = "review-queue";
		expect(validateTriggerDraft(draft)).toEqual([]);
	});

	it("requires at least 2 sub-triggers for composite", () => {
		const draft = newTriggerDraft("composite");
		expect(draft.composite?.triggers).toHaveLength(2);
		draft.composite = {
			mode: "or",
			correlationWindowSecs: "",
			triggers: [newTriggerDraft("manual")],
		};
		expect(validateTriggerDraft(draft).join(" ")).toMatch(/at least 2/);
	});

	it("rejects composite nesting beyond the depth cap", () => {
		let nested: TriggerDraft = newTriggerDraft("composite");
		for (let i = 0; i < MAX_COMPOSITE_DEPTH; i++) {
			const outer = newTriggerDraft("composite");
			outer.composite = {
				mode: "or",
				correlationWindowSecs: "",
				triggers: [nested, newTriggerDraft("manual")],
			};
			nested = outer;
		}
		expect(validateTriggerDraft(nested).join(" ")).toMatch(/maximum depth/);
	});

	it("validates sub-triggers recursively", () => {
		const draft = newTriggerDraft("composite");
		const github = newTriggerDraft("github_issues");
		draft.composite = {
			mode: "or",
			correlationWindowSecs: "",
			triggers: [github, newTriggerDraft("manual")],
		};
		expect(validateTriggerDraft(draft).join(" ")).toMatch(/Owner is required/);
	});
});

describe("newTriggerDraft", () => {
	it.each(
		ALL_TYPES.filter((t) => t !== "composite") as TriggerType[],
	)("%s default draft converts to a config of the same type", (type) => {
		expect(draftToConfig(newTriggerDraft(type)).type).toBe(type);
	});
});
