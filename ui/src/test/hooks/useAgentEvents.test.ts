/**
 * Tests for useAgentEvents hook.
 */

import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useAgentEvents } from "@/hooks/useAgentEvents";
import { agentEventBus } from "@/services/eventBus";
import type { AgentStatusChangeEvent } from "@/types/orchestrator";

describe("useAgentEvents", () => {
	it("returns a subscribe function", () => {
		const { result } = renderHook(() => useAgentEvents());
		expect(typeof result.current.subscribe).toBe("function");
	});

	it("receives events of the subscribed type", () => {
		const received: string[] = [];

		const { result } = renderHook(() => useAgentEvents());

		act(() => {
			result.current.subscribe<AgentStatusChangeEvent>(
				"agent:status_change",
				(event) => {
					received.push(event.agentId);
				},
			);
		});

		act(() => {
			agentEventBus.emit<AgentStatusChangeEvent>({
				type: "agent:status_change",
				agentId: "test-agent",
				status: "running",
				timestamp: new Date().toISOString(),
			});
		});

		expect(received).toContain("test-agent");
	});

	it("does not receive events of a different type", () => {
		const received: unknown[] = [];

		const { result } = renderHook(() => useAgentEvents());

		act(() => {
			result.current.subscribe("approval:requested", (event) => {
				received.push(event);
			});
		});

		act(() => {
			agentEventBus.emit<AgentStatusChangeEvent>({
				type: "agent:status_change",
				agentId: "irrelevant",
				status: "stopped",
				timestamp: new Date().toISOString(),
			});
		});

		expect(received).toHaveLength(0);
	});

	it("subscribe cleanup function removes the handler", () => {
		const received: string[] = [];

		const { result } = renderHook(() => useAgentEvents());

		let cleanup: (() => void) | undefined;
		act(() => {
			cleanup = result.current.subscribe<AgentStatusChangeEvent>(
				"agent:status_change",
				(event) => {
					received.push(event.agentId);
				},
			);
		});

		// Emit once — should receive
		act(() => {
			agentEventBus.emit<AgentStatusChangeEvent>({
				type: "agent:status_change",
				agentId: "first",
				status: "running",
				timestamp: new Date().toISOString(),
			});
		});

		// Remove subscription
		act(() => {
			cleanup?.();
		});

		// Emit again — should not receive
		act(() => {
			agentEventBus.emit<AgentStatusChangeEvent>({
				type: "agent:status_change",
				agentId: "second",
				status: "failed",
				timestamp: new Date().toISOString(),
			});
		});

		expect(received).toEqual(["first"]);
	});

	it("unsubscribes all handlers on unmount", () => {
		const handler = vi.fn();

		const { result, unmount } = renderHook(() => useAgentEvents());

		act(() => {
			result.current.subscribe("agent:status_change", handler);
		});

		unmount();

		act(() => {
			agentEventBus.emit<AgentStatusChangeEvent>({
				type: "agent:status_change",
				agentId: "after-unmount",
				status: "stopped",
				timestamp: new Date().toISOString(),
			});
		});

		expect(handler).not.toHaveBeenCalled();
	});
});
