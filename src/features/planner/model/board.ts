import type { Project } from "@/lib/ipc/projects";

/** The v1 status vocabulary, in workflow order — one board column each. */
export const BOARD_COLUMNS = [
	"idea",
	"shooting",
	"editing",
	"published",
] as const;

export type BoardColumn = (typeof BOARD_COLUMNS)[number];

const COLUMN_SET: ReadonlySet<string> = new Set(BOARD_COLUMNS);

/** Whether a raw status string is one of the four board columns. */
export function isBoardColumn(status: string): status is BoardColumn {
	return COLUMN_SET.has(status);
}

/** Most-recently-touched first; rows never touched sort last. */
function byTouchedDesc(a: Project, b: Project): number {
	const at = a.last_touched_at ?? "";
	const bt = b.last_touched_at ?? "";
	if (at === bt) return 0;
	return at < bt ? 1 : -1;
}

/**
 * Bucket projects into the four board columns. A row whose status is outside the
 * v1 vocabulary falls back to `idea` — folders are truth, so a stale status
 * string badges the project into the first column rather than dropping it (no
 * console noise). Each column is ordered most-recently-touched first.
 */
export function groupByStatus(
	projects: Project[],
): Record<BoardColumn, Project[]> {
	const groups: Record<BoardColumn, Project[]> = {
		idea: [],
		shooting: [],
		editing: [],
		published: [],
	};
	for (const project of projects) {
		const column = isBoardColumn(project.status) ? project.status : "idea";
		groups[column].push(project);
	}
	for (const column of BOARD_COLUMNS) {
		groups[column].sort(byTouchedDesc);
	}
	return groups;
}
