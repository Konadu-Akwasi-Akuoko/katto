import type { BoardColumn } from "@/features/planner/model/board";
import { isBoardColumn } from "@/features/planner/model/board";

/**
 * The token-class quartet a status/priority renders with. `dot`/`tint` are
 * background utilities (the chip dot and the chip fill), `fg` is the text/icon
 * colour. Class strings are literals here so the Tailwind scanner emits them.
 */
export type Appearance = {
	label: string;
	fg: string;
	dot: string;
	tint: string;
};

const STATUS: Record<BoardColumn, Appearance> = {
	idea: {
		label: "Idea",
		fg: "text-status-idea",
		dot: "bg-status-idea",
		tint: "bg-status-idea-tint",
	},
	shooting: {
		label: "Shooting",
		fg: "text-status-shooting",
		dot: "bg-status-shooting",
		tint: "bg-status-shooting-tint",
	},
	editing: {
		label: "Editing",
		fg: "text-status-editing",
		dot: "bg-status-editing",
		tint: "bg-status-editing-tint",
	},
	published: {
		label: "Published",
		fg: "text-status-published",
		dot: "bg-status-published",
		tint: "bg-status-published-tint",
	},
};

/**
 * Appearance for a project status. An unknown status falls back to `idea`,
 * mirroring the board's bucketing — folders are truth, so a stale status string
 * badges into the first column rather than dropping the project.
 */
export function statusAppearance(status: string): Appearance {
	return STATUS[isBoardColumn(status) ? status : "idea"];
}

/** The priority axis, low→high, plus the unset sentinel. */
export const PRIORITY_LEVELS = ["none", "low", "medium", "high"] as const;
export type PriorityLevel = (typeof PRIORITY_LEVELS)[number];

const PRIORITY_SET: ReadonlySet<string> = new Set(PRIORITY_LEVELS);

/** Whether a raw string is one of the four priority levels. */
export function isPriorityLevel(value: string): value is PriorityLevel {
	return PRIORITY_SET.has(value);
}

const PRIORITY: Record<Exclude<PriorityLevel, "none">, Appearance> = {
	low: {
		label: "Low",
		fg: "text-priority-low",
		dot: "bg-priority-low",
		tint: "bg-priority-low-tint",
	},
	medium: {
		label: "Medium",
		fg: "text-priority-medium",
		dot: "bg-priority-medium",
		tint: "bg-priority-medium-tint",
	},
	high: {
		label: "High",
		fg: "text-priority-high",
		dot: "bg-priority-high",
		tint: "bg-priority-high-tint",
	},
};

/**
 * Appearance for a priority. `none` (and any unknown value) returns `null` — an
 * unprioritised project renders no priority chrome at all.
 */
export function priorityAppearance(priority: string): Appearance | null {
	if (priority === "none" || !isPriorityLevel(priority)) return null;
	return PRIORITY[priority as Exclude<PriorityLevel, "none">];
}
