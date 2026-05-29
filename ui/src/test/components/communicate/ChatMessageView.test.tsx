/**
 * Tests for ChatMessageView component.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ChatMessageView } from "@/components/communicate/ChatMessageView";
import {
	makeChatMessage,
	makeChatMessageList,
	makeParticipant,
} from "@/test/mocks/factories";

// ---------------------------------------------------------------------------
// Scroll simulation helpers
// ---------------------------------------------------------------------------

/**
 * Mock scroll geometry on a container so handleScroll can compute distances.
 * distanceFromBottom = scrollHeight - scrollTop - clientHeight
 */
function mockScrollGeometry(
	el: HTMLElement,
	{
		scrollHeight,
		scrollTop,
		clientHeight,
	}: { scrollHeight: number; scrollTop: number; clientHeight: number },
) {
	Object.defineProperty(el, "scrollHeight", {
		value: scrollHeight,
		configurable: true,
	});
	Object.defineProperty(el, "scrollTop", {
		value: scrollTop,
		writable: true,
		configurable: true,
	});
	Object.defineProperty(el, "clientHeight", {
		value: clientHeight,
		configurable: true,
	});
}

const noop = () => {};

// scrollIntoView is not implemented in jsdom — provide a mock.
const scrollIntoViewMock = vi.fn();
beforeEach(() => {
	scrollIntoViewMock.mockClear();
	window.HTMLElement.prototype.scrollIntoView = scrollIntoViewMock;
});

describe("ChatMessageView", () => {
	it("renders messages", () => {
		const messages = makeChatMessageList(3);
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
			/>,
		);

		messages.forEach((msg) => {
			expect(screen.getByText(msg.content)).toBeInTheDocument();
			expect(screen.getByText(msg.sender_name)).toBeInTheDocument();
		});
	});

	it("shows loading spinner when loading", () => {
		render(
			<ChatMessageView
				messages={[]}
				loading={true}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
			/>,
		);

		// Spinner is present (no messages shown)
		expect(screen.queryByRole("region")).not.toBeInTheDocument();
		// The container with aria-label="Chat messages" should not be present
		expect(screen.queryByLabelText("Chat messages")).not.toBeInTheDocument();
	});

	it("shows empty state when no messages", () => {
		render(
			<ChatMessageView
				messages={[]}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
			/>,
		);

		expect(screen.getByText(/no messages yet/i)).toBeInTheDocument();
	});

	it('shows "beginning of conversation" when hasMore is false and messages exist', () => {
		const messages = makeChatMessageList(2);
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
			/>,
		);

		expect(screen.getByText(/beginning of conversation/i)).toBeInTheDocument();
	});

	it("shows agent and human kind badges", () => {
		const agentMsg = makeChatMessage({
			sender_kind: "agent",
			sender_name: "MyAgent",
		});
		const humanMsg = makeChatMessage({
			sender_kind: "human",
			sender_name: "MyHuman",
		});

		render(
			<ChatMessageView
				messages={[agentMsg, humanMsg]}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
			/>,
		);

		const badges = screen.getAllByText(/agent|human/i);
		// At least one 'agent' badge and one 'human' badge
		expect(badges.some((b) => b.textContent === "agent")).toBe(true);
		expect(badges.some((b) => b.textContent === "human")).toBe(true);
	});

	it("shows reply indicator when reply_to is set", () => {
		const parent = makeChatMessage({ content: "Original message" });
		const reply = makeChatMessage({
			reply_to: parent.id,
			content: "Reply message",
		});

		render(
			<ChatMessageView
				messages={[parent, reply]}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
			/>,
		);

		// The parent content appears in the reply indicator
		expect(screen.getAllByText("Original message")).toHaveLength(2);
		expect(screen.getByText("Reply message")).toBeInTheDocument();
	});

	it("scrolls to bottom instantly on initial load", () => {
		const messages = makeChatMessageList(5);
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		expect(scrollIntoViewMock).toHaveBeenCalledWith({ behavior: "instant" });
	});

	it("does not scroll to bottom while loading", () => {
		render(
			<ChatMessageView
				messages={[]}
				loading={true}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		// Loading spinner is shown, scrollIntoView should not have been called
		// with the initial-scroll behavior because messages haven't arrived yet.
		const instantCalls = scrollIntoViewMock.mock.calls.filter(
			(call) => call[0]?.behavior === "instant",
		);
		expect(instantCalls).toHaveLength(0);
	});

	it("scrolls to bottom again when switching rooms", () => {
		const messagesRoom1 = makeChatMessageList(3);
		const messagesRoom2 = makeChatMessageList(3);

		const { rerender } = render(
			<ChatMessageView
				messages={messagesRoom1}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		const callsAfterRoom1 = scrollIntoViewMock.mock.calls.filter(
			(call) => call[0]?.behavior === "instant",
		).length;
		expect(callsAfterRoom1).toBe(1);

		// Simulate room switch: loading starts with empty messages, then resolves.
		rerender(
			<ChatMessageView
				messages={[]}
				loading={true}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-2"
			/>,
		);

		rerender(
			<ChatMessageView
				messages={messagesRoom2}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-2"
			/>,
		);

		const callsAfterRoom2 = scrollIntoViewMock.mock.calls.filter(
			(call) => call[0]?.behavior === "instant",
		).length;
		expect(callsAfterRoom2).toBe(2);
	});

	// -------------------------------------------------------------------------
	// Scroll-lock / jump-to-latest button
	// -------------------------------------------------------------------------

	it("does not show jump-to-latest button when at bottom", () => {
		const messages = makeChatMessageList(5);
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		expect(
			screen.queryByRole("button", { name: /jump to latest/i }),
		).not.toBeInTheDocument();
	});

	it("shows jump-to-latest button when scrolled away from bottom", () => {
		const messages = makeChatMessageList(5);
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		const container = screen.getByLabelText("Chat messages");

		// Simulate scrolling well above the bottom (distanceFromBottom = 700 > 120)
		mockScrollGeometry(container, {
			scrollHeight: 1000,
			scrollTop: 0,
			clientHeight: 300,
		});
		fireEvent.scroll(container);

		expect(
			screen.getByRole("button", { name: /jump to latest/i }),
		).toBeInTheDocument();
	});

	it("hides jump-to-latest button when scrolled back to bottom", () => {
		const messages = makeChatMessageList(5);
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		const container = screen.getByLabelText("Chat messages");

		// Scroll up — lock engages
		mockScrollGeometry(container, {
			scrollHeight: 1000,
			scrollTop: 0,
			clientHeight: 300,
		});
		fireEvent.scroll(container);
		expect(
			screen.getByRole("button", { name: /jump to latest/i }),
		).toBeInTheDocument();

		// Scroll back to bottom — lock releases (distanceFromBottom = 10 < 120)
		mockScrollGeometry(container, {
			scrollHeight: 1000,
			scrollTop: 690,
			clientHeight: 300,
		});
		fireEvent.scroll(container);
		expect(
			screen.queryByRole("button", { name: /jump to latest/i }),
		).not.toBeInTheDocument();
	});

	it("clicking jump-to-latest scrolls to bottom and hides the button", async () => {
		const user = userEvent.setup();
		const messages = makeChatMessageList(5);
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		const container = screen.getByLabelText("Chat messages");

		// Scroll up to show the button
		mockScrollGeometry(container, {
			scrollHeight: 1000,
			scrollTop: 0,
			clientHeight: 300,
		});
		fireEvent.scroll(container);

		const btn = screen.getByRole("button", { name: /jump to latest/i });
		await user.click(btn);

		// Button should be gone after clicking
		expect(
			screen.queryByRole("button", { name: /jump to latest/i }),
		).not.toBeInTheDocument();
	});

	it("shows new message count on jump-to-latest button when messages arrive while locked", async () => {
		const messages = makeChatMessageList(3);
		const { rerender } = render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		// Scroll up to engage lock
		const container = screen.getByLabelText("Chat messages");
		mockScrollGeometry(container, {
			scrollHeight: 1000,
			scrollTop: 0,
			clientHeight: 300,
		});
		fireEvent.scroll(container);

		expect(
			screen.getByRole("button", { name: /jump to latest/i }),
		).toBeInTheDocument();

		// Two new messages arrive
		const newMsg1 = makeChatMessage({});
		const newMsg2 = makeChatMessage({});
		rerender(
			<ChatMessageView
				messages={[...messages, newMsg1, newMsg2]}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		expect(
			screen.getByRole("button", { name: /2 new messages/i }),
		).toBeInTheDocument();
	});

	it("resets scroll lock and new message count when switching rooms", async () => {
		const user = userEvent.setup();
		const messages = makeChatMessageList(3);
		const { rerender } = render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		// Scroll up and accumulate new messages in room-1
		const container = screen.getByLabelText("Chat messages");
		mockScrollGeometry(container, {
			scrollHeight: 1000,
			scrollTop: 0,
			clientHeight: 300,
		});
		fireEvent.scroll(container);
		rerender(
			<ChatMessageView
				messages={[...messages, makeChatMessage({})]}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);
		expect(
			screen.getByRole("button", { name: /new message/i }),
		).toBeInTheDocument();

		// Switch to room-2 — all scroll state should reset
		rerender(
			<ChatMessageView
				messages={makeChatMessageList(2)}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-2"
			/>,
		);

		expect(
			screen.queryByRole("button", { name: /jump to latest/i }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /new message/i }),
		).not.toBeInTheDocument();

		// Suppress unused variable warning
		void user;
	});

	it("does not scroll to bottom when loading older messages", () => {
		const messages = makeChatMessageList(5);
		const olderMessages = [...makeChatMessageList(5), ...messages];

		const { rerender } = render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={true}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		const callsAfterInitial = scrollIntoViewMock.mock.calls.filter(
			(call) => call[0]?.behavior === "instant",
		).length;
		expect(callsAfterInitial).toBe(1);

		// Simulate loading older messages (prepend to list, loadingOlder transitions)
		rerender(
			<ChatMessageView
				messages={olderMessages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				roomId="room-1"
			/>,
		);

		// Still only 1 instant-scroll call — older-message prepend must not jump to bottom
		const callsAfterOlder = scrollIntoViewMock.mock.calls.filter(
			(call) => call[0]?.behavior === "instant",
		).length;
		expect(callsAfterOlder).toBe(1);
	});

	// -------------------------------------------------------------------------
	// ThinkingIndicator
	// -------------------------------------------------------------------------

	it("shows no thinking indicator when busyAgents is empty", () => {
		const messages = makeChatMessageList(3);
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				busyAgents={[]}
			/>,
		);

		expect(screen.queryByText(/is thinking/i)).not.toBeInTheDocument();
		expect(screen.queryByText(/are thinking/i)).not.toBeInTheDocument();
	});

	it("shows thinking indicator for a single busy agent", () => {
		const messages = makeChatMessageList(3);
		const agent = makeParticipant({ kind: "agent", display_name: "Planner" });
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				busyAgents={[agent]}
			/>,
		);

		expect(screen.getByText("Planner is thinking…")).toBeInTheDocument();
	});

	it("shows combined label for two busy agents", () => {
		const messages = makeChatMessageList(3);
		const a1 = makeParticipant({ kind: "agent", display_name: "Alpha" });
		const a2 = makeParticipant({ kind: "agent", display_name: "Beta" });
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				busyAgents={[a1, a2]}
			/>,
		);

		expect(
			screen.getByText("Alpha and Beta are thinking…"),
		).toBeInTheDocument();
	});

	it("shows generic label when three or more agents are busy", () => {
		const messages = makeChatMessageList(3);
		const agents = [
			makeParticipant({ kind: "agent", display_name: "A" }),
			makeParticipant({ kind: "agent", display_name: "B" }),
			makeParticipant({ kind: "agent", display_name: "C" }),
		];
		render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				busyAgents={agents}
			/>,
		);

		expect(screen.getByText("3 agents are thinking…")).toBeInTheDocument();
	});

	it("thinking indicator disappears when busyAgents becomes empty", () => {
		const messages = makeChatMessageList(3);
		const agent = makeParticipant({ kind: "agent", display_name: "Planner" });

		const { rerender } = render(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				busyAgents={[agent]}
			/>,
		);

		expect(screen.getByText("Planner is thinking…")).toBeInTheDocument();

		rerender(
			<ChatMessageView
				messages={messages}
				loading={false}
				loadingOlder={false}
				hasMore={false}
				onLoadOlder={noop}
				busyAgents={[]}
			/>,
		);

		expect(screen.queryByText(/is thinking/i)).not.toBeInTheDocument();
	});
});
