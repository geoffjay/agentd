/**
 * autoLayout tests.
 *
 * Covers: empty graph, single pair, multiple pairs, shared agent,
 * unconnected nodes, LR vs TB direction, and option overrides.
 */

import { describe, expect, it } from "vitest";
import type { Edge, Node } from "@xyflow/react";
import { autoLayout } from "./layout";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

function makeNode(id: string, type: "trigger" | "agent" | "other"): Node {
	return {
		id,
		type,
		position: { x: 0, y: 0 },
		data: {},
	};
}

function makeEdge(id: string, source: string, target: string): Edge {
	return { id, source, target };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("autoLayout", () => {
	describe("empty and trivial cases", () => {
		it("returns empty array unchanged", () => {
			expect(autoLayout([], [])).toEqual([]);
		});

		it("returns single trigger with no edges at origin", () => {
			const nodes = [makeNode("t1", "trigger")];
			const result = autoLayout(nodes, []);
			expect(result[0].position).toEqual({ x: 80, y: 0 });
		});

		it("returns single agent with no edges at agent column", () => {
			const nodes = [makeNode("a1", "agent")];
			const result = autoLayout(nodes, []);
			// Agent with no triggers: group size = 1, slot = 0
			expect(result[0].position).toEqual({ x: 380, y: 0 });
		});
	});

	describe("LR direction (default)", () => {
		it("places trigger at TRIGGER_X and agent at AGENT_X for single pair", () => {
			const nodes = [makeNode("t1", "trigger"), makeNode("a1", "agent")];
			const edges = [makeEdge("e1", "t1", "a1")];
			const result = autoLayout(nodes, edges);
			const t1 = result.find((n) => n.id === "t1")!;
			const a1 = result.find((n) => n.id === "a1")!;
			expect(t1.position.x).toBe(80);
			expect(a1.position.x).toBe(380); // 80 + 300
		});

		it("centers agent beside its single trigger", () => {
			const nodes = [makeNode("t1", "trigger"), makeNode("a1", "agent")];
			const edges = [makeEdge("e1", "t1", "a1")];
			const result = autoLayout(nodes, edges);
			const t1 = result.find((n) => n.id === "t1")!;
			const a1 = result.find((n) => n.id === "a1")!;
			// Single trigger in group: agent should have same y as trigger
			expect(a1.position.y).toBe(t1.position.y);
		});

		it("centers agent when two triggers connect to it", () => {
			const nodes = [
				makeNode("t1", "trigger"),
				makeNode("t2", "trigger"),
				makeNode("a1", "agent"),
			];
			const edges = [
				makeEdge("e1", "t1", "a1"),
				makeEdge("e2", "t2", "a1"),
			];
			const result = autoLayout(nodes, edges);
			const t1 = result.find((n) => n.id === "t1")!;
			const t2 = result.find((n) => n.id === "t2")!;
			const a1 = result.find((n) => n.id === "a1")!;
			// Agent y should be midpoint of t1 and t2 y positions
			const expectedY = (t1.position.y + t2.position.y) / 2;
			expect(a1.position.y).toBe(expectedY);
		});

		it("separates two agent groups vertically", () => {
			const nodes = [
				makeNode("t1", "trigger"),
				makeNode("a1", "agent"),
				makeNode("t2", "trigger"),
				makeNode("a2", "agent"),
			];
			const edges = [
				makeEdge("e1", "t1", "a1"),
				makeEdge("e2", "t2", "a2"),
			];
			const result = autoLayout(nodes, edges);
			const a1 = result.find((n) => n.id === "a1")!;
			const a2 = result.find((n) => n.id === "a2")!;
			// Groups are separated by a gap slot, so a2 must be below a1
			expect(a2.position.y).toBeGreaterThan(a1.position.y);
		});

		it("places unconnected trigger below connected groups", () => {
			const nodes = [
				makeNode("t1", "trigger"),
				makeNode("a1", "agent"),
				makeNode("t2", "trigger"), // unconnected
			];
			const edges = [makeEdge("e1", "t1", "a1")];
			const result = autoLayout(nodes, edges);
			const t1 = result.find((n) => n.id === "t1")!;
			const t2 = result.find((n) => n.id === "t2")!;
			expect(t2.position.y).toBeGreaterThan(t1.position.y);
		});

		it("respects nodeSpacing option", () => {
			const nodes = [
				makeNode("t1", "trigger"),
				makeNode("t2", "trigger"),
				makeNode("a1", "agent"),
			];
			const edges = [
				makeEdge("e1", "t1", "a1"),
				makeEdge("e2", "t2", "a1"),
			];
			const result = autoLayout(nodes, edges, { nodeSpacing: 200 });
			const t1 = result.find((n) => n.id === "t1")!;
			const t2 = result.find((n) => n.id === "t2")!;
			expect(t2.position.y - t1.position.y).toBe(200);
		});

		it("respects rankSpacing option", () => {
			const nodes = [makeNode("t1", "trigger"), makeNode("a1", "agent")];
			const edges = [makeEdge("e1", "t1", "a1")];
			const result = autoLayout(nodes, edges, { rankSpacing: 500 });
			const t1 = result.find((n) => n.id === "t1")!;
			const a1 = result.find((n) => n.id === "a1")!;
			expect(a1.position.x - t1.position.x).toBe(500);
		});

		it("passes 'other' typed nodes through unchanged", () => {
			const other = { ...makeNode("o1", "other"), position: { x: 999, y: 888 } };
			const nodes = [makeNode("t1", "trigger"), other];
			const result = autoLayout(nodes, []);
			const o1 = result.find((n) => n.id === "o1")!;
			expect(o1.position).toEqual({ x: 999, y: 888 });
		});
	});

	describe("TB direction", () => {
		it("places triggers at top row and agents below", () => {
			const nodes = [makeNode("t1", "trigger"), makeNode("a1", "agent")];
			const edges = [makeEdge("e1", "t1", "a1")];
			const result = autoLayout(nodes, edges, { direction: "TB" });
			const t1 = result.find((n) => n.id === "t1")!;
			const a1 = result.find((n) => n.id === "a1")!;
			expect(t1.position.y).toBeLessThan(a1.position.y);
		});

		it("centers agent horizontally below its trigger group", () => {
			const nodes = [
				makeNode("t1", "trigger"),
				makeNode("t2", "trigger"),
				makeNode("a1", "agent"),
			];
			const edges = [
				makeEdge("e1", "t1", "a1"),
				makeEdge("e2", "t2", "a1"),
			];
			const result = autoLayout(nodes, edges, { direction: "TB" });
			const t1 = result.find((n) => n.id === "t1")!;
			const t2 = result.find((n) => n.id === "t2")!;
			const a1 = result.find((n) => n.id === "a1")!;
			const expectedX = (t1.position.x + t2.position.x) / 2;
			expect(a1.position.x).toBe(expectedX);
		});
	});

	describe("shared agent (multiple triggers, one agent)", () => {
		it("stacks three triggers vertically for one agent in LR", () => {
			const nodes = [
				makeNode("t1", "trigger"),
				makeNode("t2", "trigger"),
				makeNode("t3", "trigger"),
				makeNode("a1", "agent"),
			];
			const edges = [
				makeEdge("e1", "t1", "a1"),
				makeEdge("e2", "t2", "a1"),
				makeEdge("e3", "t3", "a1"),
			];
			const result = autoLayout(nodes, edges);
			const triggers = result.filter((n) => n.type === "trigger");
			const ys = triggers.map((n) => n.position.y).sort((a, b) => a - b);
			// Triggers should be evenly spaced
			expect(ys[1] - ys[0]).toBe(ys[2] - ys[1]);
		});
	});

	describe("duplicate edge prevention", () => {
		it("does not double-count a trigger connected by two edges to same agent", () => {
			const nodes = [makeNode("t1", "trigger"), makeNode("a1", "agent")];
			// Two edges from t1 to a1 — should not place t1 twice
			const edges = [
				makeEdge("e1", "t1", "a1"),
				makeEdge("e2", "t1", "a1"),
			];
			const result = autoLayout(nodes, edges);
			const triggers = result.filter((n) => n.type === "trigger");
			expect(triggers).toHaveLength(1);
		});
	});
});
