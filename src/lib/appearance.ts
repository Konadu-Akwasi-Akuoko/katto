import type { PriorityLevel } from "@/lib/ipc/projects";
import type { ProjectStatus } from "@/lib/project-status";
import { isProjectStatus } from "@/lib/project-status";

export type { PriorityLevel };

/**
 * The token-class pair a status/priority renders with: `fg` is the text/icon
 * colour, `tint` the chip fill. The chip's dot inherits colour from `fg` via
 * `bg-current` (see `Badge`'s `[&>.dot]:bg-current`), so there is no separate
 * dot class here. Class strings are literals here so the Tailwind scanner
 * emits them.
 */
export type Appearance = {
	label: string;
	fg: string;
	tint: string;
};

const STATUS: Record<ProjectStatus, Appearance> = {
	idea: {
		label: "Idea",
		fg: "text-status-idea",
		tint: "bg-status-idea-tint",
	},
	shooting: {
		label: "Shooting",
		fg: "text-status-shooting",
		tint: "bg-status-shooting-tint",
	},
	editing: {
		label: "Editing",
		fg: "text-status-editing",
		tint: "bg-status-editing-tint",
	},
	published: {
		label: "Published",
		fg: "text-status-published",
		tint: "bg-status-published-tint",
	},
};

/**
 * Appearance for a project status. An unknown status falls back to `idea`,
 * mirroring the board's bucketing — folders are truth, so a stale status string
 * badges into the first column rather than dropping the project.
 */
export function statusAppearance(status: string): Appearance {
	return STATUS[isProjectStatus(status) ? status : "idea"];
}

/** The priority axis, low→high, plus the unset sentinel. Ordered for menus; the
 *  vocabulary itself comes from Rust via the generated union. */
export const PRIORITY_LEVELS: readonly PriorityLevel[] = [
	"none",
	"low",
	"medium",
	"high",
];

/**
 * Menu labels for the priority axis. Separate from {@link priorityAppearance},
 * which returns null for "none" because a project renders no chrome for an unset
 * priority — a menu still has to offer "none" as a real, choosable value. Shared
 * so the board's context menu and the peek's control cannot drift apart.
 * Exhaustive over the generated union: a new Rust variant breaks this.
 */
export const PRIORITY_MENU_LABELS: Record<PriorityLevel, string> = {
	none: "None",
	low: "Low",
	medium: "Medium",
	high: "High",
};

const PRIORITY_SET: ReadonlySet<string> = new Set(PRIORITY_LEVELS);

/** Whether a raw string is one of the four priority levels. */
export function isPriorityLevel(value: string): value is PriorityLevel {
	return PRIORITY_SET.has(value);
}

const PRIORITY: Record<Exclude<PriorityLevel, "none">, Appearance> = {
	low: {
		label: "Low",
		fg: "text-priority-low",
		tint: "bg-priority-low-tint",
	},
	medium: {
		label: "Medium",
		fg: "text-priority-medium",
		tint: "bg-priority-medium-tint",
	},
	high: {
		label: "High",
		fg: "text-priority-high",
		tint: "bg-priority-high-tint",
	},
};

/**
 * Appearance for a priority. `none` (and any unknown value) returns `null` — an
 * unprioritised project renders no priority chrome at all.
 */
export function priorityAppearance(priority: string): Appearance | null {
	if (!isPriorityLevel(priority) || priority === "none") return null;
	return PRIORITY[priority];
}
