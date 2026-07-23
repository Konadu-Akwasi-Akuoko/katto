import type {
	RowId,
	ScheduleEntry,
	ScheduleKind,
} from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { RowId, ScheduleEntry, ScheduleKind };

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
 * Pin a project to a shoot/publish date. One pin per `(projectSlug, kind)`, so
 * this inserts or moves it in place, mirroring the date into the manifest + row.
 */
export const upsertScheduleEntry = (
	projectSlug: string,
	kind: ScheduleKind,
	date: string,
	note: string | null = null,
): Promise<ScheduleEntry> =>
	unwrap(commands.upsertScheduleEntry(projectSlug, kind, date, note));

/** Clear a project's shoot or publish pin. */
export const deleteScheduleEntry = (
	projectSlug: string,
	kind: ScheduleKind,
): Promise<null> => unwrap(commands.deleteScheduleEntry(projectSlug, kind));
