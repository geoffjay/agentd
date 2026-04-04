/**
 * RoomList — sidebar component for browsing communicate rooms.
 *
 * Features:
 * - Displays room name, type badge, and topic
 * - Highlights the selected room
 * - Search/filter by name
 * - Unread indicator based on last-read timestamp stored in localStorage
 * - Loading skeleton while fetching
 */

import { Hash, Lock, Radio, Search } from "lucide-react";
import { useState } from "react";
import type { Room, RoomType } from "@/types/communicate";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const LAST_READ_KEY = "agentd:communicate:last-read";

function getLastRead(): Record<string, string> {
	try {
		return JSON.parse(localStorage.getItem(LAST_READ_KEY) ?? "{}");
	} catch {
		return {};
	}
}

export function markRoomAsRead(roomId: string): void {
	const lastRead = getLastRead();
	lastRead[roomId] = new Date().toISOString();
	localStorage.setItem(LAST_READ_KEY, JSON.stringify(lastRead));
}

function isUnread(room: Room): boolean {
	const lastRead = getLastRead();
	const readAt = lastRead[room.id];
	if (!readAt) return false;
	return new Date(room.updated_at) > new Date(readAt);
}

// ---------------------------------------------------------------------------
// Room type icon
// ---------------------------------------------------------------------------

function RoomTypeIcon({ type }: { type: RoomType }) {
	switch (type) {
		case "direct":
			return <Lock size={14} className="shrink-0 text-th-text-muted" />;
		case "broadcast":
			return <Radio size={14} className="shrink-0 text-th-status-warning-text" />;
		default:
			return <Hash size={14} className="shrink-0 text-th-text-muted" />;
	}
}

// ---------------------------------------------------------------------------
// Single room item
// ---------------------------------------------------------------------------

interface RoomItemProps {
	room: Room;
	selected: boolean;
	onClick: () => void;
}

function RoomItem({ room, selected, onClick }: RoomItemProps) {
	const unread = isUnread(room);

	return (
		<button
			type="button"
			onClick={onClick}
			className={[
				"w-full flex items-start gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors",
				selected
					? "bg-th-accent text-th-accent-text"
					: "text-th-text-secondary hover:bg-th-surface-hover hover:text-th-text",
			].join(" ")}
		>
			<span className="mt-0.5">
				<RoomTypeIcon type={room.room_type} />
			</span>
			<span className="min-w-0 flex-1">
				<span className="flex items-center gap-1.5">
					<span className="truncate font-medium">{room.name}</span>
					{unread && !selected && (
						<span
							className="h-2 w-2 shrink-0 rounded-full bg-th-accent"
							aria-label="Unread messages"
						/>
					)}
				</span>
				{room.topic && (
					<span className="block truncate text-xs text-th-text-muted mt-0.5">
						{room.topic}
					</span>
				)}
			</span>
		</button>
	);
}

// ---------------------------------------------------------------------------
// RoomList
// ---------------------------------------------------------------------------

interface RoomListProps {
	rooms: Room[];
	selectedRoomId: string | undefined;
	loading: boolean;
	onSelectRoom: (room: Room) => void;
}

export function RoomList({
	rooms,
	selectedRoomId,
	loading,
	onSelectRoom,
}: RoomListProps) {
	const [search, setSearch] = useState("");

	const filtered = search
		? rooms.filter((r) => r.name.toLowerCase().includes(search.toLowerCase()))
		: rooms;

	return (
		<div className="flex h-full flex-col">
			{/* Search */}
			<div className="px-3 py-2">
				<div className="relative">
					<Search
						size={14}
						className="absolute left-2.5 top-1/2 -translate-y-1/2 text-th-text-muted pointer-events-none"
					/>
					<input
						type="search"
						placeholder="Find a room…"
						value={search}
						onChange={(e) => setSearch(e.target.value)}
						className="w-full rounded-md bg-th-surface-raised pl-8 pr-3 py-1.5 text-sm text-th-text placeholder-th-text-muted focus:outline-none focus:ring-2 focus:ring-th-focus-ring"
						aria-label="Search rooms"
					/>
				</div>
			</div>

			{/* List */}
			<nav
				aria-label="Rooms"
				className="flex-1 overflow-y-auto px-2 py-1 space-y-0.5"
			>
				{loading ? (
					// Skeleton
					Array.from({ length: 5 }).map((_, i) => (
						<div
							key={i}
							className="h-10 rounded-md bg-th-surface-raised animate-pulse mx-1"
							aria-hidden="true"
						/>
					))
				) : filtered.length === 0 ? (
					<p className="px-3 py-4 text-center text-xs text-th-text-muted">
						{search ? "No rooms match your search." : "No rooms yet."}
					</p>
				) : (
					filtered.map((room) => (
						<RoomItem
							key={room.id}
							room={room}
							selected={room.id === selectedRoomId}
							onClick={() => onSelectRoom(room)}
						/>
					))
				)}
			</nav>
		</div>
	);
}
