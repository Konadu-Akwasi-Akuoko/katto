import type { CalendarMarker } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { CalendarMarker };

export const calendarKeys = {
	all: ["calendar"] as const,
	range: (from: string, to: string) => ["calendar", from, to] as const,
};

/** All calendar markers whose day falls in `[from, to]` (inclusive ISO dates). */
export const listCalendar = (
	from: string,
	to: string,
): Promise<CalendarMarker[]> => unwrap(commands.listCalendar(from, to));
