import { SystemAgentList } from "@/components/agents/SystemAgentList";
import { AgentList } from "./agents/AgentList";

/**
 * AgentsPage — top-level agents page.
 *
 * Renders two sections:
 * 1. **System Agents** — built-in agents managed by the orchestrator.
 *    Always-present, no create/delete actions.
 * 2. **Agents** — user-created agents with full CRUD and filtering.
 */
export function AgentsPage() {
	return (
		<>
			<SystemAgentList />
			<AgentList />
		</>
	);
}

export default AgentsPage;
