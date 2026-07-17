import type { Project } from "@/lib/ipc/projects";
import type { ProjectStatus } from "@/lib/project-status";
import { isProjectStatus, PROJECT_STATUSES } from "@/lib/project-status";

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
): Record<ProjectStatus, Project[]> {
	const groups: Record<ProjectStatus, Project[]> = {
		idea: [],
		shooting: [],
		editing: [],
		published: [],
	};
	for (const project of projects) {
		const column = isProjectStatus(project.status) ? project.status : "idea";
		groups[column].push(project);
	}
	for (const column of PROJECT_STATUSES) {
		groups[column].sort(byTouchedDesc);
	}
	return groups;
}
