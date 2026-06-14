/**
 * Client for the Orchestrator service (default port 17006).
 *
 * Manages agent lifecycle, tool policies, approvals, and exposes
 * WebSocket helpers for real-time agent output streaming.
 */

import type {
	BackendInfo,
	HealthResponse,
	PaginatedResponse,
} from "@/types/common";
import type {
	AddDirRequest,
	AddDirResponse,
	Agent,
	AgentUsageStats,
	ApprovalActionRequest,
	ClearContextResponse,
	CreateAgentRequest,
	CreateWorkflowRequest,
	DispatchRecord,
	ListAgentsParams,
	ListApprovalsParams,
	PendingApproval,
	SendMessageRequest,
	SendMessageResponse,
	SetModelRequest,
	ToolPolicy,
	TriggerWorkflowRequest,
	UpdateAgentRequest,
	UpdateAgentResponse,
	UpdatePolicyRequest,
	UpdateWorkflowRequest,
	Workflow,
} from "@/types/orchestrator";
import { ApiClient, withAuth } from "./base";
import { serviceConfig } from "./config";

/**
 * Normalize a workflow payload from the API.
 *
 * The orchestrator emits `trigger_config`; older versions emitted
 * `source_config`. Accept both so the UI works against either.
 */
function normalizeWorkflow(
	raw: Workflow & { source_config?: unknown },
): Workflow {
	if (!raw.trigger_config && raw.source_config) {
		return {
			...raw,
			trigger_config: raw.source_config as Workflow["trigger_config"],
		};
	}
	return raw;
}

export class OrchestratorClient extends ApiClient {
	// -------------------------------------------------------------------------
	// Health
	// -------------------------------------------------------------------------

	getHealth(): Promise<HealthResponse> {
		return this.get<HealthResponse>("/health");
	}

	/** `GET /info` — active backend type and capabilities. */
	getInfo(): Promise<BackendInfo> {
		return this.get<BackendInfo>("/info");
	}

	// -------------------------------------------------------------------------
	// Agents
	// -------------------------------------------------------------------------

	listAgents(params?: ListAgentsParams): Promise<PaginatedResponse<Agent>> {
		return this.get<PaginatedResponse<Agent>>(
			"/agents",
			params as Record<string, string>,
		);
	}

	/**
	 * `GET /system-agents` — list built-in system agents.
	 *
	 * Returns only agents with `built_in = true`. These are spawned automatically
	 * by the orchestrator at startup and are always present while the service runs.
	 */
	listSystemAgents(): Promise<Agent[]> {
		return this.get<Agent[]>("/system-agents");
	}

	createAgent(request: CreateAgentRequest): Promise<Agent> {
		return this.post<Agent>("/agents", request);
	}

	getAgent(id: string): Promise<Agent> {
		return this.get<Agent>(`/agents/${id}`);
	}

	deleteAgent(id: string): Promise<Agent> {
		return this.delete<Agent>(`/agents/${id}`);
	}

	restartAgent(id: string): Promise<Agent> {
		return this.post<Agent>(`/agents/${id}/restart`, {});
	}

	/**
	 * `PATCH /agents/{id}` — update an agent's configuration.
	 *
	 * Merge-patch semantics: absent fields are unchanged. Pass
	 * `restart: true` to relaunch the process so launch-affecting changes
	 * apply immediately. See {@link UpdateAgentRequest} for env redaction
	 * round-trip rules.
	 */
	updateAgent(
		id: string,
		request: UpdateAgentRequest,
	): Promise<UpdateAgentResponse> {
		return this.patch<UpdateAgentResponse>(`/agents/${id}`, request);
	}

	// -------------------------------------------------------------------------
	// Agent actions
	// -------------------------------------------------------------------------

	sendMessage(agentId: string, message: string): Promise<SendMessageResponse> {
		const body: SendMessageRequest = { content: message };
		// Waking a dormant built-in agent holds the request server-side for up
		// to ~30s while the spawned session connects, so use a longer timeout
		// than the 10s client default.
		return this.post<SendMessageResponse>(`/agents/${agentId}/message`, body, {
			timeoutMs: 45_000,
		});
	}

	updateModel(agentId: string, request: SetModelRequest): Promise<Agent> {
		return this.put<Agent>(`/agents/${agentId}/model`, request);
	}

	// -------------------------------------------------------------------------
	// Additional directory management
	// -------------------------------------------------------------------------

	addDir(agentId: string, path: string): Promise<AddDirResponse> {
		const body: AddDirRequest = { path };
		return this.post<AddDirResponse>(`/agents/${agentId}/dirs`, body);
	}

	removeDir(agentId: string, path: string): Promise<AddDirResponse> {
		const body: AddDirRequest = { path };
		return this.deleteWithBody<AddDirResponse>(`/agents/${agentId}/dirs`, body);
	}

	// -------------------------------------------------------------------------
	// Usage & context management
	// -------------------------------------------------------------------------

	getAgentUsage(agentId: string): Promise<AgentUsageStats> {
		return this.get<AgentUsageStats>(`/agents/${agentId}/usage`);
	}

	clearContext(agentId: string): Promise<ClearContextResponse> {
		return this.post<ClearContextResponse>(
			`/agents/${agentId}/clear-context`,
			{},
		);
	}

	// -------------------------------------------------------------------------
	// Tool policy
	// -------------------------------------------------------------------------

	getPolicy(agentId: string): Promise<ToolPolicy> {
		return this.get<ToolPolicy>(`/agents/${agentId}/policy`);
	}

	updatePolicy(
		agentId: string,
		policy: UpdatePolicyRequest,
	): Promise<ToolPolicy> {
		return this.put<ToolPolicy>(`/agents/${agentId}/policy`, policy);
	}

	// -------------------------------------------------------------------------
	// Approvals
	// -------------------------------------------------------------------------

	listApprovals(
		params?: ListApprovalsParams,
	): Promise<PaginatedResponse<PendingApproval>> {
		return this.get<PaginatedResponse<PendingApproval>>(
			"/approvals",
			params as Record<string, string>,
		);
	}

	listAgentApprovals(
		agentId: string,
		params?: ListApprovalsParams,
	): Promise<PaginatedResponse<PendingApproval>> {
		return this.get<PaginatedResponse<PendingApproval>>(
			`/agents/${agentId}/approvals`,
			params as Record<string, string>,
		);
	}

	getApproval(id: string): Promise<PendingApproval> {
		return this.get<PendingApproval>(`/approvals/${id}`);
	}

	approveRequest(
		id: string,
		body?: ApprovalActionRequest,
	): Promise<PendingApproval> {
		return this.post<PendingApproval>(`/approvals/${id}/approve`, body ?? {});
	}

	denyRequest(
		id: string,
		body?: ApprovalActionRequest,
	): Promise<PendingApproval> {
		return this.post<PendingApproval>(`/approvals/${id}/deny`, body ?? {});
	}

	// -------------------------------------------------------------------------
	// Workflows
	// -------------------------------------------------------------------------

	async listWorkflows(params?: {
		limit?: number;
		offset?: number;
	}): Promise<PaginatedResponse<Workflow>> {
		const page = await this.get<PaginatedResponse<Workflow>>(
			"/workflows",
			params as Record<string, string>,
		);
		return { ...page, items: page.items.map(normalizeWorkflow) };
	}

	async getWorkflow(id: string): Promise<Workflow> {
		return normalizeWorkflow(await this.get<Workflow>(`/workflows/${id}`));
	}

	async createWorkflow(request: CreateWorkflowRequest): Promise<Workflow> {
		return normalizeWorkflow(await this.post<Workflow>("/workflows", request));
	}

	async updateWorkflow(
		id: string,
		request: UpdateWorkflowRequest,
	): Promise<Workflow> {
		return normalizeWorkflow(
			await this.put<Workflow>(`/workflows/${id}`, request),
		);
	}

	deleteWorkflow(id: string): Promise<void> {
		return this.delete<void>(`/workflows/${id}`);
	}

	getWorkflowHistory(
		id: string,
		params?: { limit?: number; offset?: number },
	): Promise<PaginatedResponse<DispatchRecord>> {
		return this.get<PaginatedResponse<DispatchRecord>>(
			`/workflows/${id}/history`,
			params as Record<string, string>,
		);
	}

	/**
	 * Manually trigger a workflow, bypassing its normal trigger strategy.
	 * Used by the dispatch retry modal to re-run a dispatch with
	 * (optionally edited) variables.
	 */
	triggerWorkflow(
		id: string,
		request?: TriggerWorkflowRequest,
	): Promise<DispatchRecord> {
		return this.post<DispatchRecord>(`/workflows/${id}/trigger`, request ?? {});
	}

	// -------------------------------------------------------------------------
	// WebSocket streaming
	// -------------------------------------------------------------------------

	/**
	 * Opens a read-only WebSocket to stream output from a specific agent.
	 * URL: ws://<host>/stream/<agentId>
	 *
	 * NOTE: Do NOT connect to /ws/<agentId> — that endpoint is reserved for
	 * the agent's Claude CLI process. Connecting to it replaces the agent's
	 * connection in the registry, severing communication with the actual agent.
	 */
	connectAgentStream(agentId: string): WebSocket {
		return this.openWebSocket(`/stream/${agentId}`);
	}

	/**
	 * Opens a WebSocket to monitor all agents.
	 * URL: ws://<host>/stream
	 */
	connectAllStream(): WebSocket {
		return this.openWebSocket("/stream");
	}

	/**
	 * Opens a WebSocket to monitor a specific agent.
	 * URL: ws://<host>/stream/<agentId>
	 */
	connectAgentMonitor(agentId: string): WebSocket {
		return this.openWebSocket(`/stream/${agentId}`);
	}
}

/** Singleton client instance using the configured service URL */
export const orchestratorClient = new OrchestratorClient(
	withAuth({ baseUrl: serviceConfig.orchestratorServiceUrl }),
);
