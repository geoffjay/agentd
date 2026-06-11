/**
 * agentFormModel tests — validation, request building, and the redacted
 * env round-trip behavior.
 */

import { describe, expect, it } from "vitest";
import {
	agentFormFromAgent,
	agentToCreateRequest,
	agentToUpdateRequest,
	DEFAULT_AGENT_FORM,
	hasAgentErrors,
	validateAgentForm,
} from "@/components/agents/form/agentFormModel";
import { makeAgent } from "@/test/mocks/factories";
import { ENV_REDACTED } from "@/types/orchestrator";

function validState() {
	return {
		...DEFAULT_AGENT_FORM,
		name: "my-agent",
		workingDir: "/home/user/project",
	};
}

describe("validateAgentForm", () => {
	it("accepts a minimal valid form", () => {
		expect(hasAgentErrors(validateAgentForm(validState()))).toBe(false);
	});

	it("requires name and working directory", () => {
		const errors = validateAgentForm(DEFAULT_AGENT_FORM);
		expect(errors.name).toBeTruthy();
		expect(errors.workingDir).toBeTruthy();
	});

	it("rejects a non-positive auto-clear threshold", () => {
		const errors = validateAgentForm({
			...validState(),
			autoClearThreshold: "-5",
		});
		expect(errors.autoClearThreshold).toBeTruthy();
	});

	it("rejects the redaction placeholder as an env value on create", () => {
		const errors = validateAgentForm({
			...validState(),
			env: [{ key: "API_KEY", value: ENV_REDACTED }],
		});
		expect(errors.env).toBeTruthy();
	});

	it("allows the redaction placeholder when editing", () => {
		const errors = validateAgentForm(
			{ ...validState(), env: [{ key: "API_KEY", value: ENV_REDACTED }] },
			{ editing: true },
		);
		expect(errors.env).toBeUndefined();
	});

	it("rejects mounts missing one of the two paths", () => {
		const errors = validateAgentForm({
			...validState(),
			extraMounts: [{ hostPath: "/host", containerPath: "", readOnly: false }],
		});
		expect(errors.extraMounts).toBeTruthy();
	});
});

describe("agentToCreateRequest", () => {
	it("omits empty optional fields", () => {
		const request = agentToCreateRequest(validState());
		expect(request.name).toBe("my-agent");
		expect(request.shell).toBe("/bin/sh");
		expect(request.model).toBeUndefined();
		expect(request.env).toBeUndefined();
		expect(request.rooms).toBeUndefined();
		expect(request.extra_mounts).toBeUndefined();
		expect(request.resource_limits).toBeUndefined();
	});

	it("drops the prompt when interactive", () => {
		const request = agentToCreateRequest({
			...validState(),
			interactive: true,
			prompt: "ignored",
		});
		expect(request.prompt).toBeUndefined();
		expect(request.interactive).toBe(true);
	});

	it("sends only the active system prompt source", () => {
		const inline = agentToCreateRequest({
			...validState(),
			systemPrompt: "You are…",
			systemPromptFile: "/ignored.md",
		});
		expect(inline.system_prompt).toBe("You are…");
		expect(inline.system_prompt_file).toBeUndefined();

		const file = agentToCreateRequest({
			...validState(),
			systemPromptMode: "file",
			systemPrompt: "ignored",
			systemPromptFile: "/prompt.md",
		});
		expect(file.system_prompt).toBeUndefined();
		expect(file.system_prompt_file).toBe("/prompt.md");
	});
});

describe("agentToUpdateRequest", () => {
	it("passes redacted env values through for the server-side sentinel merge", () => {
		const request = agentToUpdateRequest({
			...validState(),
			env: [
				{ key: "KEPT", value: ENV_REDACTED },
				{ key: "NEW", value: "fresh" },
			],
		});
		expect(request.env).toEqual({ KEPT: ENV_REDACTED, NEW: "fresh" });
	});

	it("clears the inactive system prompt field explicitly", () => {
		const request = agentToUpdateRequest({
			...validState(),
			systemPromptMode: "file",
			systemPromptFile: "/prompt.md",
		});
		expect(request.system_prompt).toBe("");
		expect(request.system_prompt_file).toBe("/prompt.md");
	});

	it("includes the restart flag when requested", () => {
		expect(agentToUpdateRequest(validState()).restart).toBe(false);
		expect(agentToUpdateRequest(validState(), { restart: true }).restart).toBe(
			true,
		);
	});
});

describe("agentFormFromAgent", () => {
	it("round-trips an agent's config into the form state", () => {
		const agent = makeAgent({
			name: "loaded",
			config: {
				working_dir: "/work",
				shell: "/bin/zsh",
				interactive: false,
				worktree: true,
				system_prompt_file: "/prompt.md",
				tool_policy: { mode: "deny_list", tools: ["Bash"] },
				model: "opus",
				env: { A: "***" },
				auto_clear_threshold: 50000,
				additional_dirs: ["/extra"],
				rooms: ["engineering"],
			},
		});
		const state = agentFormFromAgent(agent);
		expect(state.name).toBe("loaded");
		expect(state.workingDir).toBe("/work");
		expect(state.worktree).toBe(true);
		expect(state.systemPromptMode).toBe("file");
		expect(state.systemPromptFile).toBe("/prompt.md");
		expect(state.toolPolicy.mode).toBe("deny_list");
		expect(state.toolPolicy.toolsCsv).toBe("Bash");
		expect(state.env).toEqual([{ key: "A", value: "***" }]);
		expect(state.autoClearThreshold).toBe("50000");
		expect(state.additionalDirs).toEqual(["/extra"]);
		expect(state.rooms).toEqual(["engineering"]);
	});
});
