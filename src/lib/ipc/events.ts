import { commands } from "@/lib/ipc/bindings.gen";
import type { Event, RowId } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { Event, RowId };

export const eventsKeys = {
	all: ["events"] as const,
};

/** Most-recent-first page of activity-log events; `beforeId` pages backward. */
export const listEvents = (limit: number, beforeId: RowId | null = null): Promise<Event[]> =>
	unwrap(commands.listEvents(limit, beforeId));
