/**
 * Unit tests for the test data factories.
 * Verifies that factories produce valid, typed objects.
 */

import { beforeEach, describe, expect, it } from "vitest";
import {
	makeAgent,
	makeAgentConfig,
	makeAgentList,
	makeAnswerResponse,
	makeApprovalList,
	makeCountResponse,
	makeDeleteResponse,
	makeMemory,
	makeMemoryList,
	makeNotification,
	makeNotificationList,
	makePendingApproval,
	makePrivateMemory,
	makeQuestion,
	makeQuestionActionResponse,
	makeQuestionInfo,
	makeQuestionMemory,
	makeRequestMemory,
	makeSearchResponse,
	makeSharedMemory,
	makeTriggerResponse,
	makeUrgentNotification,
	resetAgentSeq,
	resetMemorySeq,
	resetNotificationSeq,
	resetQuestionSeq,
} from "./index";

beforeEach(() => {
	resetAgentSeq();
	resetNotificationSeq();
	resetQuestionSeq();
	resetMemorySeq();
});

// ---------------------------------------------------------------------------
// Agent factory
// ---------------------------------------------------------------------------

describe("makeAgentConfig", () => {
	it("returns a config with required fields", () => {
		const config = makeAgentConfig();
		expect(config.shell).toBe("/bin/bash");
		expect(config.tool_policy).toEqual({ mode: "allow_all" });
	});

	it("applies overrides", () => {
		const config = makeAgentConfig({
			interactive: true,
			model: "claude-3-opus",
		});
		expect(config.interactive).toBe(true);
		expect(config.model).toBe("claude-3-opus");
	});
});

describe("makeAgent", () => {
	it("returns a valid agent with a unique id", () => {
		const a1 = makeAgent();
		const a2 = makeAgent();
		expect(a1.id).not.toBe(a2.id);
	});

	it("defaults to running status", () => {
		expect(makeAgent().status).toBe("running");
	});

	it("applies overrides", () => {
		const agent = makeAgent({ name: "my-bot", status: "stopped" });
		expect(agent.name).toBe("my-bot");
		expect(agent.status).toBe("stopped");
	});

	it("has an ISO 8601 created_at timestamp", () => {
		const agent = makeAgent();
		expect(new Date(agent.created_at).toISOString()).toBe(agent.created_at);
	});
});

describe("makeAgentList", () => {
	it("creates the requested number of agents", () => {
		expect(makeAgentList(5)).toHaveLength(5);
	});

	it("each agent has a unique id", () => {
		const ids = makeAgentList(4).map((a) => a.id);
		const unique = new Set(ids);
		expect(unique.size).toBe(4);
	});

	it("applies shared overrides to every agent", () => {
		const agents = makeAgentList(3, { status: "failed" });
		expect(agents.every((a) => a.status === "failed")).toBe(true);
	});
});

describe("makePendingApproval", () => {
	it("has pending status by default", () => {
		expect(makePendingApproval().status).toBe("pending");
	});

	it("applies overrides", () => {
		const approval = makePendingApproval({ tool_name: "read_file" });
		expect(approval.tool_name).toBe("read_file");
	});
});

describe("makeApprovalList", () => {
	it("creates the requested number of approvals", () => {
		expect(makeApprovalList(2)).toHaveLength(2);
	});
});

// ---------------------------------------------------------------------------
// Notification factory
// ---------------------------------------------------------------------------

describe("makeNotification", () => {
	it("returns a valid notification with a unique id", () => {
		const n1 = makeNotification();
		const n2 = makeNotification();
		expect(n1.id).not.toBe(n2.id);
	});

	it("defaults to normal priority and pending status", () => {
		const notif = makeNotification();
		expect(notif.priority).toBe("normal");
		expect(notif.status).toBe("pending");
	});

	it("applies overrides", () => {
		const notif = makeNotification({ priority: "high", title: "Custom Title" });
		expect(notif.priority).toBe("high");
		expect(notif.title).toBe("Custom Title");
	});
});

describe("makeUrgentNotification", () => {
	it("sets priority to urgent and requires_response to true", () => {
		const notif = makeUrgentNotification();
		expect(notif.priority).toBe("urgent");
		expect(notif.requires_response).toBe(true);
	});
});

describe("makeNotificationList", () => {
	it("creates the requested number of notifications", () => {
		expect(makeNotificationList(7)).toHaveLength(7);
	});
});

describe("makeCountResponse", () => {
	it("returns a count response with correct total", () => {
		const resp = makeCountResponse(10, { Pending: 6, Viewed: 4 });
		expect(resp.total).toBe(10);
		expect(resp.by_status).toHaveLength(2);
	});
});

// ---------------------------------------------------------------------------
// Question/Ask factory
// ---------------------------------------------------------------------------

describe("makeQuestionInfo", () => {
	it("defaults to Pending status", () => {
		expect(makeQuestionInfo().status).toBe("Pending");
	});

	it("applies overrides", () => {
		const q = makeQuestionInfo({ status: "Answered", answer: "yes" });
		expect(q.status).toBe("Answered");
		expect(q.answer).toBe("yes");
	});
});

describe("makeTriggerResponse", () => {
	it("includes checks_run", () => {
		const resp = makeTriggerResponse();
		expect(resp.checks_run).toContain("TmuxSessions");
	});

	it("returns tmux_sessions result", () => {
		const resp = makeTriggerResponse();
		expect(resp.results.tmux_sessions.running).toBe(true);
	});

	it("applies overrides", () => {
		const resp = makeTriggerResponse({ notifications_sent: ["notif-1"] });
		expect(resp.notifications_sent).toContain("notif-1");
	});
});

describe("makeAnswerResponse", () => {
	it("defaults success to true", () => {
		expect(makeAnswerResponse().success).toBe(true);
	});
});

// ---------------------------------------------------------------------------
// New Question factory
// ---------------------------------------------------------------------------

describe("makeQuestion", () => {
	it("defaults to Pending status", () => {
		expect(makeQuestion().status).toBe("Pending");
	});

	it("defaults to Normal priority", () => {
		expect(makeQuestion().priority).toBe("Normal");
	});

	it("includes required fields", () => {
		const q = makeQuestion();
		expect(q.id).toBeDefined();
		expect(q.agent_id).toBeDefined();
		expect(q.category).toBeDefined();
		expect(q.question).toBeDefined();
		expect(q.asked_at).toBeDefined();
	});

	it("applies overrides", () => {
		const q = makeQuestion({
			status: "Answered",
			answer: "yes",
			priority: "High",
			category: "deployment",
		});
		expect(q.status).toBe("Answered");
		expect(q.answer).toBe("yes");
		expect(q.priority).toBe("High");
		expect(q.category).toBe("deployment");
	});
});

describe("makeQuestionActionResponse", () => {
	it("defaults success to true", () => {
		expect(makeQuestionActionResponse().success).toBe(true);
	});

	it("applies overrides", () => {
		const r = makeQuestionActionResponse({
			success: false,
			message: "Already answered",
		});
		expect(r.success).toBe(false);
		expect(r.message).toBe("Already answered");
	});
});

// ---------------------------------------------------------------------------
// Memory factory
// ---------------------------------------------------------------------------

describe("makeMemory", () => {
	it("returns a valid memory with a unique id", () => {
		const m1 = makeMemory();
		const m2 = makeMemory();
		expect(m1.id).not.toBe(m2.id);
	});

	it("has a mem_ prefixed id", () => {
		const mem = makeMemory();
		expect(mem.id).toMatch(/^mem_/);
	});

	it("defaults to information type and public visibility", () => {
		const mem = makeMemory();
		expect(mem.type).toBe("information");
		expect(mem.visibility).toBe("public");
	});

	it("applies overrides", () => {
		const mem = makeMemory({
			type: "question",
			visibility: "private",
			content: "Custom",
		});
		expect(mem.type).toBe("question");
		expect(mem.visibility).toBe("private");
		expect(mem.content).toBe("Custom");
	});

	it("has an ISO 8601 created_at timestamp", () => {
		const mem = makeMemory();
		expect(new Date(mem.created_at).toISOString()).toBe(mem.created_at);
	});
});

describe("makeMemoryList", () => {
	it("creates the requested number of memories", () => {
		expect(makeMemoryList(5)).toHaveLength(5);
	});

	it("each memory has a unique id", () => {
		const ids = makeMemoryList(4).map((m) => m.id);
		const unique = new Set(ids);
		expect(unique.size).toBe(4);
	});

	it("applies shared overrides to every memory", () => {
		const mems = makeMemoryList(3, { type: "request" });
		expect(mems.every((m) => m.type === "request")).toBe(true);
	});
});

describe("makeQuestionMemory", () => {
	it("sets type to question", () => {
		expect(makeQuestionMemory().type).toBe("question");
	});
});

describe("makeRequestMemory", () => {
	it("sets type to request", () => {
		expect(makeRequestMemory().type).toBe("request");
	});
});

describe("makePrivateMemory", () => {
	it("sets visibility to private", () => {
		expect(makePrivateMemory().visibility).toBe("private");
	});
});

describe("makeSharedMemory", () => {
	it("sets visibility to shared with shared_with list", () => {
		const mem = makeSharedMemory();
		expect(mem.visibility).toBe("shared");
		expect(mem.shared_with.length).toBeGreaterThan(0);
	});
});

describe("makeSearchResponse", () => {
	it("returns a search response with memories and total", () => {
		const resp = makeSearchResponse();
		expect(resp.memories.length).toBeGreaterThan(0);
		expect(resp.total).toBe(resp.memories.length);
	});

	it("uses provided memories", () => {
		const mem = makeMemory({ content: "Custom" });
		const resp = makeSearchResponse([mem], 1);
		expect(resp.memories).toHaveLength(1);
		expect(resp.memories[0].content).toBe("Custom");
	});
});

describe("makeDeleteResponse", () => {
	it("defaults deleted to true", () => {
		expect(makeDeleteResponse().deleted).toBe(true);
	});

	it("can set deleted to false", () => {
		expect(makeDeleteResponse(false).deleted).toBe(false);
	});
});
