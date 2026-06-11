/**
 * Agent YAML template import/export tests — field mapping, template
 * defaults, every warning class, and the import(export(state)) round
 * trip.
 */

import { describe, expect, it } from "vitest";
import {
	type AgentFormState,
	DEFAULT_AGENT_FORM,
} from "@/components/agents/form/agentFormModel";
import {
	exportAgentYaml,
	importAgentYaml,
} from "@/utils/yamlTemplates/agentTemplate";

const FULL_TEMPLATE = `
name: worker
working_dir: "/projects/app"
shell: /bin/zsh
model: claude-sonnet-4-6
worktree: true
user: agentd

tool_policy:
  mode: deny_list
  tools:
    - "Write(.agentd/agents/*)"
    - "Bash(git push --force*)"
  sandbox_bypass:
    - "Bash(git-spice *)"

env:
  ANTHROPIC_API_KEY: \${ANTHROPIC_API_KEY}
  LOG_LEVEL: \${LOG_LEVEL:-info}
  PLAIN: value

auto_clear_threshold: 100000
network_policy: isolated
docker_image: custom:latest
extra_mounts:
  - host_path: /data
    container_path: /mnt/data
    read_only: true
resource_limits:
  cpu_limit: 2
  memory_limit_mb: 4096
additional_dirs:
  - /shared

rooms:
  - engineering
  - name: announcements
    role: observer

system_prompt: |
  You are a worker agent.
`;

describe("importAgentYaml", () => {
	it("maps the full template into form state", () => {
		const { state } = importAgentYaml(FULL_TEMPLATE);
		expect(state.name).toBe("worker");
		expect(state.workingDir).toBe("/projects/app");
		expect(state.shell).toBe("/bin/zsh");
		expect(state.model).toBe("claude-sonnet-4-6");
		expect(state.worktree).toBe(true);
		expect(state.user).toBe("agentd");
		expect(state.toolPolicy.mode).toBe("deny_list");
		expect(state.toolPolicy.toolsCsv).toContain("Write(.agentd/agents/*)");
		expect(state.toolPolicy.sandboxBypass).toEqual(["Bash(git-spice *)"]);
		expect(state.env).toContainEqual({ key: "PLAIN", value: "value" });
		expect(state.autoClearThreshold).toBe("100000");
		expect(state.networkPolicy).toBe("isolated");
		expect(state.dockerImage).toBe("custom:latest");
		expect(state.extraMounts).toEqual([
			{ hostPath: "/data", containerPath: "/mnt/data", readOnly: true },
		]);
		expect(state.cpuLimit).toBe("2");
		expect(state.memoryLimitMb).toBe("4096");
		expect(state.additionalDirs).toEqual(["/shared"]);
		expect(state.rooms).toEqual(["engineering", "announcements"]);
		expect(state.systemPrompt).toContain("You are a worker agent.");
		expect(state.systemPromptMode).toBe("inline");
	});

	it("applies the apply.rs defaults for working_dir and shell", () => {
		const { state } = importAgentYaml("name: minimal");
		expect(state.workingDir).toBe(".");
		expect(state.shell).toBe("zsh");
	});

	it("warns about env substitution, dropped room roles, and unknown keys", () => {
		const { warnings } = importAgentYaml(`${FULL_TEMPLATE}\nbogus_key: 1\n`);
		const joined = warnings.join(" ");
		expect(joined).toMatch(/\$\{ANTHROPIC_API_KEY\}/);
		expect(joined).toMatch(/\$\{LOG_LEVEL:-info\}/);
		expect(joined).toMatch(/roles are not supported/i);
		expect(joined).toMatch(/bogus_key/);
	});

	it("selects file mode when system_prompt_file is set and warns about path resolution", () => {
		const { state, warnings } = importAgentYaml(
			"name: a\nsystem_prompt_file: prompts/sys.md",
		);
		expect(state.systemPromptMode).toBe("file");
		expect(state.systemPromptFile).toBe("prompts/sys.md");
		expect(warnings.join(" ")).toMatch(/resolved on the orchestrator host/);
	});

	it("throws on invalid YAML and non-mapping documents", () => {
		expect(() => importAgentYaml("- just\n- a list")).toThrow(/mapping/);
		expect(() => importAgentYaml("a: [unclosed")).toThrow();
	});
});

describe("exportAgentYaml", () => {
	it("omits empty optionals and includes set fields", () => {
		const { yaml } = exportAgentYaml({
			...DEFAULT_AGENT_FORM,
			name: "exported",
			workingDir: "/work",
			model: "opus",
			rooms: ["engineering"],
		});
		expect(yaml).toContain("name: exported");
		expect(yaml).toContain("working_dir: /work");
		expect(yaml).toContain("model: opus");
		expect(yaml).toContain("- engineering");
		expect(yaml).not.toContain("docker_image");
		expect(yaml).not.toContain("env:");
		expect(yaml).not.toContain("prompt:");
	});

	it("warns when redacted env values are exported", () => {
		const { warnings } = exportAgentYaml({
			...DEFAULT_AGENT_FORM,
			name: "redacted",
			workingDir: ".",
			env: [{ key: "API_KEY", value: "***" }],
		});
		expect(warnings.join(" ")).toMatch(/redacted by the API/);
	});
});

describe("round trip", () => {
	it("import(export(state)) preserves the form state", () => {
		const original: AgentFormState = {
			...DEFAULT_AGENT_FORM,
			name: "rt",
			workingDir: "/work",
			shell: "/bin/zsh",
			model: "sonnet",
			worktree: true,
			prompt: "do things",
			systemPrompt: "You are rt.",
			toolPolicy: {
				mode: "allow_list",
				toolsCsv: "Read, Grep",
				sandboxBypass: ["Bash(git-spice *)"],
			},
			env: [{ key: "A", value: "1" }],
			autoClearThreshold: "50000",
			additionalDirs: ["/extra"],
			rooms: ["eng"],
		};
		const { yaml } = exportAgentYaml(original);
		const { state } = importAgentYaml(yaml);
		expect(state).toEqual(original);
	});
});
