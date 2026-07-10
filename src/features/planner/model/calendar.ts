import type { ScheduleEntry } from "@/lib/ipc/schedule";

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
