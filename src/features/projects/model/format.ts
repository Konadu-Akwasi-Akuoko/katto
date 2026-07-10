import { formatShortDate } from "@/lib/date";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/**
 * Relative age of a subfolder's newest file for the freshness grid: "just now",
 * "45m ago", "9h ago", then an absolute "Jul 1" past a day. A null mtime (empty
 * or absent subfolder) renders as an em dash; an unparseable value echoes back.
 */
export function relativeMtime(iso: string | null, now: Date): string {
	if (iso === null) return "—";
	const then = new Date(iso);
	if (Number.isNaN(then.getTime())) return iso;
	const age = now.getTime() - then.getTime();
	if (age < MINUTE) return "just now";
	if (age < HOUR) return `${Math.floor(age / MINUTE)}m ago`;
	if (age < DAY) return `${Math.floor(age / HOUR)}h ago`;
	return formatShortDate(then.toISOString());
}

/** Compact absolute date ("Jul 10") for a project's shoot/publish ISO date. */
export function formatDate(iso: string): string {
	return formatShortDate(iso);
}
