/**
 * ParticipantPanel — displays the list of participants in the selected room.
 *
 * Features:
 * - Lists participants with kind badge (Agent/Human)
 * - Agent participants can show activity state (idle/busy) from orchestrator
 * - Human participants show online status based on WebSocket presence
 * - Loading skeleton while fetching
 */

import { Bot, Crown, Eye, User } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { mapApiError } from "@/hooks/useToast";
import { communicateClient } from "@/services/communicate";
import type {
	Participant,
	ParticipantKind,
	ParticipantRole,
} from "@/types/communicate";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function KindIcon({ kind }: { kind: ParticipantKind }) {
	return kind === "agent" ? (
		<Bot size={14} className="text-th-text-link" aria-hidden="true" />
	) : (
		<User size={14} className="text-th-status-success-text" aria-hidden="true" />
	);
}

function RoleIcon({ role }: { role: ParticipantRole }) {
	if (role === "admin")
		return <Crown size={12} className="text-th-status-warning-text" aria-label="Admin" />;
	if (role === "observer")
		return <Eye size={12} className="text-th-text-muted" aria-label="Observer" />;
	return null;
}

function ActivityDot({ state }: { state?: "idle" | "busy" }) {
	if (!state)
		return (
			<span className="h-2 w-2 rounded-full bg-th-text-faint" aria-label="Unknown" />
		);
	return (
		<span
			className={[
				"h-2 w-2 rounded-full",
				state === "busy"
					? "bg-th-status-warning-dot animate-pulse"
					: "bg-th-status-success-dot",
			].join(" ")}
			aria-label={state === "busy" ? "Busy" : "Idle"}
		/>
	);
}

// ---------------------------------------------------------------------------
// Participant item
// ---------------------------------------------------------------------------

function ParticipantItem({ participant }: { participant: Participant }) {
	return (
		<li className="flex items-center gap-2 px-3 py-1.5">
			<ActivityDot state={participant.activity_state} />
			<KindIcon kind={participant.kind} />
			<span className="min-w-0 flex-1">
				<span className="block truncate text-sm text-th-text-secondary">
					{participant.display_name}
				</span>
				<span className="block truncate text-xs text-th-text-muted">
					{participant.identifier}
				</span>
			</span>
			<RoleIcon role={participant.role} />
		</li>
	);
}

// ---------------------------------------------------------------------------
// ParticipantPanel
// ---------------------------------------------------------------------------

interface ParticipantPanelProps {
	roomId: string | undefined;
	/** Additional participants received via WebSocket (joined event). */
	realtimeParticipants?: Participant[];
	/** Identifiers of participants who left via WebSocket. */
	leftIdentifiers?: string[];
}

export function ParticipantPanel({
	roomId,
	realtimeParticipants = [],
	leftIdentifiers = [],
}: ParticipantPanelProps) {
	const [participants, setParticipants] = useState<Participant[]>([]);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | undefined>();

	const fetchParticipants = useCallback(async (id: string) => {
		setLoading(true);
		setError(undefined);
		try {
			const result = await communicateClient.listParticipants(id, {
				limit: 100,
			});
			setParticipants(result.items);
		} catch (err) {
			setError(mapApiError(err));
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		if (!roomId) {
			setParticipants([]);
			return;
		}
		fetchParticipants(roomId);
	}, [roomId, fetchParticipants]);

	// Merge realtime join/leave events
	const merged = [
		...participants.filter((p) => !leftIdentifiers.includes(p.identifier)),
		...realtimeParticipants.filter(
			(rp) => !participants.some((p) => p.identifier === rp.identifier),
		),
	];

	const agents = merged.filter((p) => p.kind === "agent");
	const humans = merged.filter((p) => p.kind === "human");

	if (!roomId) {
		return (
			<div className="flex h-full items-center justify-center p-4">
				<p className="text-xs text-th-text-muted text-center">
					Select a room to see participants.
				</p>
			</div>
		);
	}

	if (loading) {
		return (
			<div className="space-y-2 px-3 py-3">
				{Array.from({ length: 4 }).map((_, i) => (
					<div
						key={i}
						className="h-8 rounded-md bg-th-surface-raised animate-pulse"
						aria-hidden="true"
					/>
				))}
			</div>
		);
	}

	if (error) {
		return (
			<p className="px-3 py-4 text-xs text-th-status-error-text text-center">{error}</p>
		);
	}

	return (
		<div className="flex h-full flex-col overflow-y-auto">
			{agents.length > 0 && (
				<section aria-labelledby="agents-heading">
					<h3
						id="agents-heading"
						className="px-3 py-2 text-xs font-semibold uppercase tracking-wider text-th-text-muted"
					>
						Agents — {agents.length}
					</h3>
					<ul role="list" className="space-y-0.5">
						{agents.map((p) => (
							<ParticipantItem key={p.id} participant={p} />
						))}
					</ul>
				</section>
			)}

			{humans.length > 0 && (
				<section
					aria-labelledby="humans-heading"
					className={agents.length > 0 ? "mt-3" : ""}
				>
					<h3
						id="humans-heading"
						className="px-3 py-2 text-xs font-semibold uppercase tracking-wider text-th-text-muted"
					>
						Humans — {humans.length}
					</h3>
					<ul role="list" className="space-y-0.5">
						{humans.map((p) => (
							<ParticipantItem key={p.id} participant={p} />
						))}
					</ul>
				</section>
			)}

			{merged.length === 0 && (
				<p className="px-3 py-4 text-center text-xs text-th-text-muted">
					No participants yet.
				</p>
			)}
		</div>
	);
}
