import type { ScheduleEntry } from "@/lib/ipc/schedule";

const MS_PER_DAY = 86_400_000;

/** Parse an ISO `YYYY-MM-DD` date as UTC midnight, dodging local-timezone drift. */
function isoToMs(iso: string): number {
	const parts = iso.split("-");
	return Date.UTC(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2]));
}

/**
 * Entries falling in the half-open window `[today, today+7)`, sorted ascending
 * by date. Today is included; the seventh day out is not. Shared by the tray,
 * calendar, and dashboard week-ahead surfaces.
 */
export function weekAhead(
	entries: ScheduleEntry[],
	todayIso: string,
): ScheduleEntry[] {
	const start = isoToMs(todayIso);
	const end = start + 7 * MS_PER_DAY;
	return entries
		.filter((entry) => {
			const at = isoToMs(entry.date);
			return at >= start && at < end;
		})
		.sort((a, b) => (a.date < b.date ? -1 : a.date > b.date ? 1 : 0));
}
