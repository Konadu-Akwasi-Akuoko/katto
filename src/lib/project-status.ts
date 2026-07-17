/** The v1 project status vocabulary, in workflow order. */
export const PROJECT_STATUSES = [
	"idea",
	"shooting",
	"editing",
	"published",
] as const;

export type ProjectStatus = (typeof PROJECT_STATUSES)[number];

const STATUS_SET: ReadonlySet<string> = new Set(PROJECT_STATUSES);

/** Whether a raw status string is one of the four v1 statuses. */
export function isProjectStatus(status: string): status is ProjectStatus {
	return STATUS_SET.has(status);
}
