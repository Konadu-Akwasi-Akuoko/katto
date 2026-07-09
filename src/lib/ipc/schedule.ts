import type { RowId, ScheduleEntry } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { RowId, ScheduleEntry };

export const scheduleKeys = {
	all: ["schedule"] as const,
	range: (from: string, to: string) => ["schedule", from, to] as const,
};

/** Schedule entries whose date falls within `[from, to]` (inclusive ISO bounds). */
export const listSchedule = (
	from: string,
	to: string,
): Promise<ScheduleEntry[]> => unwrap(commands.listSchedule(from, to));

/**
 * Pin a project to a date. At most one entry exists per `(projectSlug, kind)`
 * pair, so this inserts or updates in place. `kind` is `shoot` or `publish`.
 */
export const upsertScheduleEntry = (
	projectSlug: string,
	kind: string,
	date: string,
	note: string | null = null,
): Promise<ScheduleEntry> =>
	unwrap(commands.upsertScheduleEntry(projectSlug, kind, date, note));

/** Remove a schedule entry by id. */
export const deleteScheduleEntry = (id: RowId): Promise<null> =>
	unwrap(commands.deleteScheduleEntry(id));
