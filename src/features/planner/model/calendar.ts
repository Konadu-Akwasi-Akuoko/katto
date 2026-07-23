import { formatShortDate } from "@/lib/date";
import type { CalendarMarker } from "@/lib/ipc/calendar";
import type { ScheduleEntry } from "@/lib/ipc/schedule";

/** Whether the calendar shows a whole month or a single Monday-first week. */
export type CalendarMode = "month" | "week";

/** The four legend/color categories a marker belongs to (its `kind`). */
export type CalendarCategory = CalendarMarker["kind"];

/** The status vocabulary, used by the phase multiselect. */
export const ALL_PHASES = ["idea", "shooting", "editing", "published"] as const;

/** The legend + phase + project filter state applied client-side. */
export interface CalendarFilters {
	categories: Record<CalendarCategory, boolean>;
	phases: readonly string[];
	project: string | null;
}

/**
 * Filter the fetched marker set: hide toggled-off categories, restrict phase
 * moves to the selected destination phases, and (when set) keep one project's
 * markers. Backlog markers have no `project_slug`, so a project filter drops them.
 */
export function applyCalendarFilters(
	markers: CalendarMarker[],
	f: CalendarFilters,
): CalendarMarker[] {
	return markers.filter((m) => {
		if (!f.categories[m.kind]) return false;
		if (m.kind === "phase" && !f.phases.includes(m.to)) return false;
		if (f.project !== null) {
			if (!("project_slug" in m)) return false;
			if (m.project_slug !== f.project) return false;
		}
		return true;
	});
}

/** Group markers by their ISO day (`YYYY-MM-DD`) for cell rendering. */
export function markersByDate(
	markers: CalendarMarker[],
): Map<string, CalendarMarker[]> {
	const map = new Map<string, CalendarMarker[]>();
	for (const m of markers) {
		const list = map.get(m.date) ?? [];
		list.push(m);
		map.set(m.date, list);
	}
	return map;
}

/** One day cell in a calendar grid. */
export interface CalendarCell {
	/** ISO date, `YYYY-MM-DD`. */
	iso: string;
	/** Day of the month, 1–31. */
	day: number;
	/** Whether the cell belongs to the month (or week anchor) being displayed. */
	inMonth: boolean;
}

const MS_PER_DAY = 86_400_000;

/** Parse an ISO `YYYY-MM-DD` date as UTC midnight, dodging local-timezone drift. */
function parseIsoUtc(iso: string): Date {
	const parts = iso.split("-");
	return new Date(
		Date.UTC(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2])),
	);
}

function isoOf(date: Date): string {
	return date.toISOString().slice(0, 10);
}

function addDays(date: Date, days: number): Date {
	return new Date(date.getTime() + days * MS_PER_DAY);
}

/** Days to step back from `date` to reach the Monday that opens its week. */
function mondayOffset(date: Date): number {
	return (date.getUTCDay() + 6) % 7;
}

function cellOf(date: Date, year: number, month: number): CalendarCell {
	return {
		iso: isoOf(date),
		day: date.getUTCDate(),
		inMonth: date.getUTCMonth() === month && date.getUTCFullYear() === year,
	};
}

/**
 * A fixed 6×7 month grid, Monday-first. `month` is 0-indexed like
 * `Date.getUTCMonth` (0 = January). Leading and trailing cells spill into the
 * neighbouring months with `inMonth: false`; the grid is always six rows so the
 * layout never reflows between months.
 */
export function monthGrid(year: number, month: number): CalendarCell[][] {
	const first = new Date(Date.UTC(year, month, 1));
	const start = addDays(first, -mondayOffset(first));
	const weeks: CalendarCell[][] = [];
	for (let week = 0; week < 6; week++) {
		const row: CalendarCell[] = [];
		for (let day = 0; day < 7; day++) {
			row.push(cellOf(addDays(start, week * 7 + day), year, month));
		}
		weeks.push(row);
	}
	return weeks;
}

/**
 * The Monday-through-Sunday week containing `anchorIso`. Cells are flagged
 * in-month against the anchor's own month, so a week straddling a month
 * boundary dims its foreign days.
 */
export function weekRow(anchorIso: string): CalendarCell[] {
	const anchor = parseIsoUtc(anchorIso);
	const monday = addDays(anchor, -mondayOffset(anchor));
	const year = anchor.getUTCFullYear();
	const month = anchor.getUTCMonth();
	const row: CalendarCell[] = [];
	for (let day = 0; day < 7; day++) {
		row.push(cellOf(addDays(monday, day), year, month));
	}
	return row;
}

/**
 * Shift an ISO date by whole months, snapping to the first of the resulting
 * month. `delta` may be negative; month/year rollover is handled by
 * `Date.UTC`'s normalisation (e.g. December + 1 → the next January).
 */
export function addMonthsIso(iso: string, delta: number): string {
	const [y, m] = iso.split("-").map(Number);
	const date = new Date(Date.UTC(y ?? 0, (m ?? 1) - 1 + delta, 1));
	return date.toISOString().slice(0, 10);
}

/**
 * Shift an ISO date by whole days, keeping the day-of-month. `delta` may be
 * negative; month and year boundaries (including leap days) are normalised by
 * `Date.UTC`.
 */
export function addDaysIso(iso: string, delta: number): string {
	const [y, m, d] = iso.split("-").map(Number);
	const date = new Date(Date.UTC(y ?? 0, (m ?? 1) - 1, (d ?? 1) + delta));
	return date.toISOString().slice(0, 10);
}

/**
 * Heading for the current view: "February 2024" in month mode; the Monday-first
 * week's span ("Feb 12 – Feb 18") in week mode. Returns an empty string only
 * for a malformed anchor that yields no week.
 */
export function periodLabel(anchorIso: string, mode: CalendarMode): string {
	if (mode === "week") {
		const row = weekRow(anchorIso);
		const start = row[0]?.iso;
		const end = row[row.length - 1]?.iso;
		if (start === undefined || end === undefined) return "";
		return `${formatShortDate(start)} – ${formatShortDate(end)}`;
	}
	const [y, m] = anchorIso.split("-").map(Number);
	return new Date(Date.UTC(y ?? 0, (m ?? 1) - 1, 1)).toLocaleDateString(
		"en-US",
		{ month: "long", year: "numeric", timeZone: "UTC" },
	);
}

/**
 * Bucket schedule entries by their ISO date. Insertion order is preserved
 * within each day, and days with no entries are simply absent from the map.
 */
export function chipsByDate(
	entries: ScheduleEntry[],
): Map<string, ScheduleEntry[]> {
	const byDate = new Map<string, ScheduleEntry[]>();
	for (const entry of entries) {
		const bucket = byDate.get(entry.date);
		if (bucket) bucket.push(entry);
		else byDate.set(entry.date, [entry]);
	}
	return byDate;
}
