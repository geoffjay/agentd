import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { PolicyDisplay } from "@/components/agents/PolicyDisplay";

describe("PolicyDisplay", () => {
	it('renders "Allow All" for allow_all policy', () => {
		render(<PolicyDisplay policy={{ mode: "allow_all" }} />);
		expect(screen.getByText("Allow All")).toBeInTheDocument();
	});

	it('renders "Deny All" for deny_all policy', () => {
		render(<PolicyDisplay policy={{ mode: "deny_all" }} />);
		expect(screen.getByText("Deny All")).toBeInTheDocument();
	});

	it('renders "Require Approval" for require_approval policy', () => {
		render(<PolicyDisplay policy={{ mode: "require_approval" }} />);
		expect(screen.getByText("Require Approval")).toBeInTheDocument();
	});

	it("renders tool badges for allow_list policy", () => {
		render(
			<PolicyDisplay
				policy={{ mode: "allow_list", tools: ["bash", "read_file"] }}
			/>,
		);
		expect(screen.getByText("Allow List")).toBeInTheDocument();
		expect(screen.getByText("bash")).toBeInTheDocument();
		expect(screen.getByText("read_file")).toBeInTheDocument();
	});

	it("renders tool badges for deny_list policy", () => {
		render(
			<PolicyDisplay policy={{ mode: "deny_list", tools: ["rm", "dd"] }} />,
		);
		expect(screen.getByText("Deny List")).toBeInTheDocument();
		expect(screen.getByText("rm")).toBeInTheDocument();
		expect(screen.getByText("dd")).toBeInTheDocument();
	});

	it('renders "None configured" for empty allow_list', () => {
		render(<PolicyDisplay policy={{ mode: "allow_list", tools: [] }} />);
		expect(screen.getByText(/none configured/i)).toBeInTheDocument();
	});

	it('renders "None configured" for empty deny_list', () => {
		render(<PolicyDisplay policy={{ mode: "deny_list", tools: [] }} />);
		expect(screen.getByText(/none configured/i)).toBeInTheDocument();
	});

	it("does not render tools section for allow_all", () => {
		render(<PolicyDisplay policy={{ mode: "allow_all" }} />);
		expect(screen.queryByText(/none configured/i)).not.toBeInTheDocument();
		expect(screen.queryByText("Tools")).not.toBeInTheDocument();
	});

	// -- sandbox_bypass --

	it("does not render sandbox bypass section when sandbox_bypass is absent", () => {
		render(<PolicyDisplay policy={{ mode: "allow_all" }} />);
		expect(screen.queryByText(/sandbox bypass/i)).not.toBeInTheDocument();
	});

	it("does not render sandbox bypass section when sandbox_bypass is empty", () => {
		render(
			<PolicyDisplay policy={{ mode: "allow_all", sandbox_bypass: [] }} />,
		);
		expect(screen.queryByText(/sandbox bypass/i)).not.toBeInTheDocument();
	});

	it("renders sandbox bypass toggle when globs are present", () => {
		render(
			<PolicyDisplay
				policy={{
					mode: "deny_list",
					tools: ["Bash(rm *)"],
					sandbox_bypass: [
						"Bash(git-spice branch submit *)",
						"Bash(gh pr create *)",
					],
				}}
			/>,
		);
		expect(screen.getByText(/sandbox bypass/i)).toBeInTheDocument();
		// Badge shows count
		expect(screen.getByText("2")).toBeInTheDocument();
		// Globs are hidden until expanded
		expect(
			screen.queryByText("Bash(git-spice branch submit *)"),
		).not.toBeInTheDocument();
	});

	it("expands sandbox bypass list on click", async () => {
		const user = userEvent.setup();
		render(
			<PolicyDisplay
				policy={{
					mode: "allow_all",
					sandbox_bypass: [
						"Bash(git-spice branch submit *)",
						"Bash(git-spice repo sync*)",
					],
				}}
			/>,
		);
		const toggle = screen.getByRole("button", {
			name: /sandbox bypass/i,
		});
		await user.click(toggle);

		expect(
			screen.getByText("Bash(git-spice branch submit *)"),
		).toBeInTheDocument();
		expect(screen.getByText("Bash(git-spice repo sync*)")).toBeInTheDocument();
	});

	it("collapses sandbox bypass list on second click", async () => {
		const user = userEvent.setup();
		render(
			<PolicyDisplay
				policy={{
					mode: "allow_all",
					sandbox_bypass: ["Bash(git-spice branch submit *)"],
				}}
			/>,
		);
		const toggle = screen.getByRole("button", {
			name: /sandbox bypass/i,
		});
		await user.click(toggle);
		expect(
			screen.getByText("Bash(git-spice branch submit *)"),
		).toBeInTheDocument();

		await user.click(toggle);
		expect(
			screen.queryByText("Bash(git-spice branch submit *)"),
		).not.toBeInTheDocument();
	});
});
