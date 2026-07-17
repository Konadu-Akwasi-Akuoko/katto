import type { ScheduleEntry } from "@/lib/ipc/schedule";

/** A schedule entry with sensible defaults; spread-override per scenario. */
export function scheduleEntry(
	overrides: Partial<ScheduleEntry> & Pick<ScheduleEntry, "date">,
): ScheduleEntry {
	return {
		id: 1,
		project_slug: "nvme-deep-dive-2026-07-08",
		kind: "shoot",
		note: null,
		...overrides,
	};
}
