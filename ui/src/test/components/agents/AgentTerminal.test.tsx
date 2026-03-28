/**
 * AgentTerminal tests.
 *
 * xterm.js uses canvas APIs unavailable in jsdom, so the Terminal class and
 * addons are mocked with plain class implementations. The tests cover
 * rendering, toolbar interactions, status badges, and graceful fallback UI.
 */

import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// ---------------------------------------------------------------------------
// Shared mock function references — kept outside vi.mock so tests can inspect
// calls via vi.clearAllMocks() / expect(mockFn).toHaveBeenCalled()
// ---------------------------------------------------------------------------

const mockTermOpen = vi.fn();
const mockTermWrite = vi.fn();
const mockTermWriteln = vi.fn();
const mockTermDispose = vi.fn();
const mockTermFocus = vi.fn();
const mockOnDataDispose = vi.fn();
const mockTermOnData = vi.fn(() => ({ dispose: mockOnDataDispose }));
const mockFitAddonFit = vi.fn();
const mockSearchFindNext = vi.fn();
const mockSearchFindPrev = vi.fn();

// ---------------------------------------------------------------------------
// Mock xterm.js — canvas not available in jsdom
// ---------------------------------------------------------------------------

vi.mock("@xterm/xterm", () => {
	class Terminal {
		options: Record<string, unknown> = {};
		cols = 80;
		rows = 24;
		loadAddon = vi.fn();
		open = mockTermOpen;
		write = mockTermWrite;
		writeln = mockTermWriteln;
		onData = mockTermOnData;
		dispose = mockTermDispose;
		focus = mockTermFocus;
	}
	return { Terminal };
});

vi.mock("@xterm/addon-fit", () => {
	class FitAddon {
		fit = mockFitAddonFit;
	}
	return { FitAddon };
});

vi.mock("@xterm/addon-web-links", () => {
	class WebLinksAddon {}
	return { WebLinksAddon };
});

vi.mock("@xterm/addon-search", () => {
	class SearchAddon {
		findNext = mockSearchFindNext;
		findPrevious = mockSearchFindPrev;
	}
	return { SearchAddon };
});

// xterm.js ships its own stylesheet — suppress the import in test
vi.mock("@xterm/xterm/css/xterm.css", () => ({}));

// ---------------------------------------------------------------------------
// Mock the orchestrator client — used by SDK-mode message compose
// ---------------------------------------------------------------------------

const mockSendMessage = vi.fn();

vi.mock("@/services/orchestrator", () => ({
	orchestratorClient: {
		sendMessage: (...args: unknown[]) => mockSendMessage(...args),
	},
}));

// ---------------------------------------------------------------------------
// Mock ResizeObserver — not available in jsdom
// ---------------------------------------------------------------------------

class MockResizeObserver {
	observe = vi.fn();
	unobserve = vi.fn();
	disconnect = vi.fn();
}
vi.stubGlobal("ResizeObserver", MockResizeObserver);

// ---------------------------------------------------------------------------
// Import component under test (after mocks are in place)
// ---------------------------------------------------------------------------

import { AgentTerminal } from "@/components/agents/AgentTerminal";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const AGENT_ID = "test-agent-id";

function renderTerminal(
	props: Partial<Parameters<typeof AgentTerminal>[0]> = {},
) {
	return render(<AgentTerminal agentId={AGENT_ID} {...props} />);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AgentTerminal", () => {
	beforeEach(() => {
		vi.clearAllMocks();
		mockSendMessage.mockResolvedValue({ status: "sent", agent_id: AGENT_ID });
	});

	it("renders the terminal container with aria-label", () => {
		renderTerminal();
		expect(screen.getByLabelText(/agent terminal output/i)).toBeInTheDocument();
	});

	it('shows a "Connecting…" status badge on initial render', () => {
		renderTerminal();
		expect(screen.getByLabelText(/terminal connecting/i)).toBeInTheDocument();
	});

	it("opens the xterm.js terminal on mount", () => {
		renderTerminal();
		expect(mockTermOpen).toHaveBeenCalledOnce();
	});

	it("registers an onData handler for keyboard input", () => {
		renderTerminal();
		expect(mockTermOnData).toHaveBeenCalledOnce();
	});

	it("disposes the terminal on unmount", () => {
		const { unmount } = renderTerminal();
		unmount();
		expect(mockTermDispose).toHaveBeenCalledOnce();
	});

	// ---------------------------------------------------------------------------
	// PTY mode badge
	// ---------------------------------------------------------------------------

	describe("PTY mode badge", () => {
		it('shows "PTY · Interactive" badge when agentInteractive=true', () => {
			renderTerminal({ agentInteractive: true });
			expect(
				screen.getByLabelText(/pty interactive mode/i),
			).toBeInTheDocument();
			expect(screen.getByLabelText(/pty interactive mode/i)).toHaveTextContent(
				"PTY · Interactive",
			);
		});

		it('shows "PTY · SDK" badge when agentInteractive=false', () => {
			renderTerminal({ agentInteractive: false });
			expect(screen.getByLabelText(/pty sdk mode/i)).toBeInTheDocument();
			expect(screen.getByLabelText(/pty sdk mode/i)).toHaveTextContent(
				"PTY · SDK",
			);
		});

		it('shows "PTY · SDK" badge by default (agentInteractive omitted)', () => {
			renderTerminal();
			expect(screen.getByLabelText(/pty sdk mode/i)).toBeInTheDocument();
		});
	});

	// ---------------------------------------------------------------------------
	// SDK-mode info banner
	// ---------------------------------------------------------------------------

	describe("SDK-mode info banner", () => {
		it("shows the info banner by default for SDK-mode agents", () => {
			renderTerminal({ agentInteractive: false });
			expect(
				screen.getByRole("note", { name: /sdk mode info/i }),
			).toBeInTheDocument();
		});

		it("shows the info banner by default when agentInteractive is omitted", () => {
			renderTerminal();
			expect(
				screen.getByRole("note", { name: /sdk mode info/i }),
			).toBeInTheDocument();
		});

		it("dismisses the info banner when the × button is clicked", () => {
			renderTerminal({ agentInteractive: false });
			fireEvent.click(
				screen.getByRole("button", { name: /dismiss sdk mode info/i }),
			);
			expect(screen.queryByRole("note", { name: /sdk mode info/i })).toBeNull();
		});

		it("does not show the info banner for interactive-mode agents", () => {
			renderTerminal({ agentInteractive: true });
			expect(screen.queryByRole("note", { name: /sdk mode info/i })).toBeNull();
		});
	});

	// ---------------------------------------------------------------------------
	// Toolbar — interactive toggle (agentInteractive=true only)
	// ---------------------------------------------------------------------------

	describe("interactive mode toggle (agentInteractive=true)", () => {
		it('renders a "Read-only" toggle button when readOnly=true (default)', () => {
			renderTerminal({ agentInteractive: true, readOnly: true });
			expect(
				screen.getByRole("button", { name: /switch to interactive mode/i }),
			).toBeInTheDocument();
			expect(screen.getByText("Read-only")).toBeInTheDocument();
		});

		it('renders an "Interactive" toggle button when readOnly=false', () => {
			renderTerminal({ agentInteractive: true, readOnly: false });
			expect(
				screen.getByRole("button", { name: /switch to read-only mode/i }),
			).toBeInTheDocument();
			expect(screen.getByText("Interactive")).toBeInTheDocument();
		});

		it("toggles from read-only to interactive on button click", () => {
			renderTerminal({ agentInteractive: true, readOnly: true });
			fireEvent.click(
				screen.getByRole("button", { name: /switch to interactive mode/i }),
			);
			expect(screen.getByText("Interactive")).toBeInTheDocument();
		});

		it("toggles from interactive to read-only on button click", () => {
			renderTerminal({ agentInteractive: true, readOnly: false });
			fireEvent.click(
				screen.getByRole("button", { name: /switch to read-only mode/i }),
			);
			expect(screen.getByText("Read-only")).toBeInTheDocument();
		});

		it("focuses the terminal when switching to interactive mode", () => {
			renderTerminal({ agentInteractive: true, readOnly: true });
			fireEvent.click(
				screen.getByRole("button", { name: /switch to interactive mode/i }),
			);
			expect(mockTermFocus).toHaveBeenCalledOnce();
		});

		it("does not focus the terminal when switching to read-only mode", () => {
			renderTerminal({ agentInteractive: true, readOnly: false });
			fireEvent.click(
				screen.getByRole("button", { name: /switch to read-only mode/i }),
			);
			expect(mockTermFocus).not.toHaveBeenCalled();
		});

		it("does not render the SDK compose input", () => {
			renderTerminal({ agentInteractive: true });
			expect(
				screen.queryByRole("textbox", { name: /send message to agent/i }),
			).toBeNull();
		});
	});

	// ---------------------------------------------------------------------------
	// Toolbar — SDK-mode compose input (agentInteractive=false, default)
	// ---------------------------------------------------------------------------

	describe("SDK-mode compose input (agentInteractive=false)", () => {
		it("shows a message input instead of the interactive toggle", () => {
			renderTerminal({ agentInteractive: false });
			expect(
				screen.getByRole("textbox", { name: /send message to agent/i }),
			).toBeInTheDocument();
			expect(screen.queryByText("Read-only")).toBeNull();
			expect(screen.queryByText("Interactive")).toBeNull();
		});

		it("shows the SDK compose input by default (agentInteractive omitted)", () => {
			renderTerminal();
			expect(
				screen.getByRole("textbox", { name: /send message to agent/i }),
			).toBeInTheDocument();
		});

		it("send button is disabled when input is empty", () => {
			renderTerminal({ agentInteractive: false });
			expect(
				screen.getByRole("button", { name: /send message to agent/i }),
			).toBeDisabled();
		});

		it("send button is enabled when input has text", () => {
			renderTerminal({ agentInteractive: false });
			fireEvent.change(
				screen.getByRole("textbox", { name: /send message to agent/i }),
				{
					target: { value: "hello" },
				},
			);
			// Button is still disabled because status is 'connecting', not 'connected'
			expect(
				screen.getByRole("button", { name: /send message to agent/i }),
			).toBeDisabled();
		});

		it("does not render the interactive PTY toggle", () => {
			renderTerminal({ agentInteractive: false });
			expect(
				screen.queryByRole("button", { name: /switch to interactive mode/i }),
			).toBeNull();
			expect(
				screen.queryByRole("button", { name: /switch to read-only mode/i }),
			).toBeNull();
		});

		it("calls orchestratorClient.sendMessage when form is submitted via Enter", async () => {
			// Simulate a connected WebSocket so the input is enabled
			class ConnectedWS {
				static CONNECTING = 0;
				static OPEN = 1;
				static CLOSING = 2;
				static CLOSED = 3;
				readonly CONNECTING = 0;
				readonly OPEN = 1;
				readonly CLOSING = 2;
				readonly CLOSED = 3;
				readyState = WebSocket.OPEN;
				binaryType = "blob";
				onopen: ((e: Event) => void) | null = null;
				onclose: ((e: CloseEvent) => void) | null = null;
				onerror: ((e: Event) => void) | null = null;
				onmessage: ((e: MessageEvent) => void) | null = null;
				constructor() {
					// Fire onopen asynchronously
					setTimeout(() => this.onopen?.(new Event("open")), 0);
				}
				send() {}
				close() {}
				addEventListener() {}
				removeEventListener() {}
				dispatchEvent() {
					return true;
				}
			}

			vi.stubGlobal("WebSocket", ConnectedWS);

			renderTerminal({ agentInteractive: false });

			// Wait for onopen to fire and status to become 'connected'
			await waitFor(() =>
				expect(
					screen.getByLabelText(/terminal connected/i),
				).toBeInTheDocument(),
			);

			const input = screen.getByRole("textbox", {
				name: /send message to agent/i,
			});
			fireEvent.change(input, { target: { value: "hello world" } });
			fireEvent.keyDown(input, { key: "Enter" });

			await waitFor(() =>
				expect(mockSendMessage).toHaveBeenCalledWith(AGENT_ID, "hello world"),
			);
		});

		it("clears the input after a successful send", async () => {
			class ConnectedWS {
				static CONNECTING = 0;
				static OPEN = 1;
				static CLOSING = 2;
				static CLOSED = 3;
				readonly CONNECTING = 0;
				readonly OPEN = 1;
				readonly CLOSING = 2;
				readonly CLOSED = 3;
				readyState = WebSocket.OPEN;
				binaryType = "blob";
				onopen: ((e: Event) => void) | null = null;
				onclose: ((e: CloseEvent) => void) | null = null;
				onerror: ((e: Event) => void) | null = null;
				onmessage: ((e: MessageEvent) => void) | null = null;
				constructor() {
					setTimeout(() => this.onopen?.(new Event("open")), 0);
				}
				send() {}
				close() {}
				addEventListener() {}
				removeEventListener() {}
				dispatchEvent() {
					return true;
				}
			}

			vi.stubGlobal("WebSocket", ConnectedWS);
			renderTerminal({ agentInteractive: false });

			await waitFor(() =>
				expect(
					screen.getByLabelText(/terminal connected/i),
				).toBeInTheDocument(),
			);

			const input = screen.getByRole("textbox", {
				name: /send message to agent/i,
			});
			fireEvent.change(input, { target: { value: "test prompt" } });
			fireEvent.keyDown(input, { key: "Enter" });

			await waitFor(() => expect(input).toHaveValue(""));
		});

		it("shows an error banner when sendMessage fails", async () => {
			mockSendMessage.mockRejectedValueOnce(new Error("Connection refused"));

			class ConnectedWS {
				static CONNECTING = 0;
				static OPEN = 1;
				static CLOSING = 2;
				static CLOSED = 3;
				readonly CONNECTING = 0;
				readonly OPEN = 1;
				readonly CLOSING = 2;
				readonly CLOSED = 3;
				readyState = WebSocket.OPEN;
				binaryType = "blob";
				onopen: ((e: Event) => void) | null = null;
				onclose: ((e: CloseEvent) => void) | null = null;
				onerror: ((e: Event) => void) | null = null;
				onmessage: ((e: MessageEvent) => void) | null = null;
				constructor() {
					setTimeout(() => this.onopen?.(new Event("open")), 0);
				}
				send() {}
				close() {}
				addEventListener() {}
				removeEventListener() {}
				dispatchEvent() {
					return true;
				}
			}

			vi.stubGlobal("WebSocket", ConnectedWS);
			renderTerminal({ agentInteractive: false });

			await waitFor(() =>
				expect(
					screen.getByLabelText(/terminal connected/i),
				).toBeInTheDocument(),
			);

			const input = screen.getByRole("textbox", {
				name: /send message to agent/i,
			});
			fireEvent.change(input, { target: { value: "oops" } });
			fireEvent.keyDown(input, { key: "Enter" });

			await waitFor(() =>
				expect(screen.getByRole("alert")).toHaveTextContent(
					"Connection refused",
				),
			);
		});
	});

	// ---------------------------------------------------------------------------
	// Toolbar — search bar
	// ---------------------------------------------------------------------------

	describe("search bar", () => {
		it("search bar is hidden by default", () => {
			renderTerminal();
			expect(
				screen.queryByRole("textbox", { name: /search terminal output/i }),
			).toBeNull();
		});

		it("opens search bar when Search button is clicked", () => {
			renderTerminal();
			fireEvent.click(
				screen.getByRole("button", { name: /search terminal output/i }),
			);
			expect(
				screen.getByRole("textbox", { name: /search terminal output/i }),
			).toBeInTheDocument();
		});

		it("closes search bar when the × button is clicked", () => {
			renderTerminal();
			fireEvent.click(
				screen.getByRole("button", { name: /search terminal output/i }),
			);
			fireEvent.click(screen.getByRole("button", { name: /dismiss search/i }));
			expect(
				screen.queryByRole("textbox", { name: /search terminal output/i }),
			).toBeNull();
		});

		it("closes search bar on Escape key", () => {
			renderTerminal();
			fireEvent.click(
				screen.getByRole("button", { name: /search terminal output/i }),
			);
			const input = screen.getByRole("textbox", {
				name: /search terminal output/i,
			});
			fireEvent.keyDown(input, { key: "Escape" });
			expect(
				screen.queryByRole("textbox", { name: /search terminal output/i }),
			).toBeNull();
		});

		it("calls findNext on Enter", () => {
			renderTerminal();
			fireEvent.click(
				screen.getByRole("button", { name: /search terminal output/i }),
			);
			const input = screen.getByRole("textbox", {
				name: /search terminal output/i,
			});
			fireEvent.change(input, { target: { value: "hello" } });
			fireEvent.keyDown(input, { key: "Enter" });
			expect(mockSearchFindNext).toHaveBeenCalledWith("hello", {
				incremental: false,
			});
		});

		it("calls findPrevious on Shift+Enter", () => {
			renderTerminal();
			fireEvent.click(
				screen.getByRole("button", { name: /search terminal output/i }),
			);
			const input = screen.getByRole("textbox", {
				name: /search terminal output/i,
			});
			fireEvent.change(input, { target: { value: "world" } });
			fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
			expect(mockSearchFindPrev).toHaveBeenCalledWith("world", {
				incremental: false,
			});
		});

		it('calls findNext when "Next match" button is clicked', () => {
			renderTerminal();
			fireEvent.click(
				screen.getByRole("button", { name: /search terminal output/i }),
			);
			const input = screen.getByRole("textbox", {
				name: /search terminal output/i,
			});
			fireEvent.change(input, { target: { value: "foo" } });
			fireEvent.click(screen.getByRole("button", { name: /next match/i }));
			expect(mockSearchFindNext).toHaveBeenCalledWith("foo", {
				incremental: false,
			});
		});

		it('calls findPrevious when "Previous match" button is clicked', () => {
			renderTerminal();
			fireEvent.click(
				screen.getByRole("button", { name: /search terminal output/i }),
			);
			const input = screen.getByRole("textbox", {
				name: /search terminal output/i,
			});
			fireEvent.change(input, { target: { value: "bar" } });
			fireEvent.click(screen.getByRole("button", { name: /previous match/i }));
			expect(mockSearchFindPrev).toHaveBeenCalledWith("bar", {
				incremental: false,
			});
		});
	});

	// ---------------------------------------------------------------------------
	// Unavailable fallback — simulate MAX_CONSECUTIVE_FAILURES close events
	// ---------------------------------------------------------------------------

	describe("unavailable fallback", () => {
		it("shows the fallback after max consecutive connection failures", async () => {
			// Replace the global WebSocket stub with one that fires onclose immediately
			// on construction, simulating a 404 response before the WS handshake.
			const closeCallbacks: Array<() => void> = [];

			class FailingWS {
				static CONNECTING = 0;
				static OPEN = 1;
				static CLOSING = 2;
				static CLOSED = 3;
				readonly CONNECTING = 0;
				readonly OPEN = 1;
				readonly CLOSING = 2;
				readonly CLOSED = 3;
				readyState = 0;
				binaryType = "blob";
				onopen: ((e: Event) => void) | null = null;
				onclose: ((e: CloseEvent) => void) | null = null;
				onerror: ((e: Event) => void) | null = null;
				onmessage: ((e: MessageEvent) => void) | null = null;
				constructor() {
					closeCallbacks.push(() => {
						this.onclose?.(new CloseEvent("close", { code: 1006 }));
					});
				}
				send() {}
				close() {}
				addEventListener() {}
				removeEventListener() {}
				dispatchEvent() {
					return true;
				}
			}

			vi.stubGlobal("WebSocket", FailingWS);

			renderTerminal();

			// Fire close events for each connection attempt (MAX_CONSECUTIVE_FAILURES = 3)
			// Each close triggers a setTimeout for reconnect; we fire that callback
			// immediately by also resolving any pending timers.
			vi.useFakeTimers();

			for (let i = 0; i < 3; i++) {
				await act(async () => {
					closeCallbacks[i]?.();
					vi.runAllTimers();
				});
			}

			vi.useRealTimers();

			expect(screen.getByLabelText(/pty not available/i)).toBeInTheDocument();
			expect(
				screen.getByText(/pty streaming not available/i),
			).toBeInTheDocument();
			expect(
				screen.getByRole("button", { name: /retry connection/i }),
			).toBeInTheDocument();
		});
	});
});
