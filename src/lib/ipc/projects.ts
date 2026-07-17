import type {
	FolderFreshness,
	InvalidManifest,
	PriorityLevel,
	Project,
	ProjectDetail,
	ReconcileReport,
} from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type {
	FolderFreshness,
	InvalidManifest,
	PriorityLevel,
	Project,
	ProjectDetail,
	ReconcileReport,
};

export const projectsKeys = {
	all: ["projects"] as const,
	detail: (slug: string) => ["projects", "detail", slug] as const,
};

/**
 * Reconcile the projects index against the studio-root folders (folders are
 * truth): adds newly-created folders, removes vanished ones, and reports any
 * invalid manifests without touching their rows.
 */
export const rescanProjects = (): Promise<ReconcileReport> =>
	unwrap(commands.rescanProjects());

/** Every project row, most-recently-touched first. */
export const listProjects = (): Promise<Project[]> =>
	unwrap(commands.listProjects());

/** One project with its manifest-validity flag and per-subfolder freshness. */
export const getProject = (slug: string): Promise<ProjectDetail> =>
	unwrap(commands.getProject(slug));

/** Create a project folder + row; `shootDate` is optional (ISO date). */
export const createProject = (
	title: string,
	shootDate: string | null = null,
): Promise<Project> => unwrap(commands.createProject(title, shootDate));

/** Move a project through the status vocabulary (writes manifest + row). */
export const setProjectStatus = (slug: string, status: string): Promise<null> =>
	unwrap(commands.setProjectStatus(slug, status));

/** Set a project's priority (writes manifest + row). */
export const setProjectPriority = (
	slug: string,
	priority: PriorityLevel,
): Promise<null> => unwrap(commands.setProjectPriority(slug, priority));

/** Set or clear a project's shoot and publish dates (writes manifest + row). */
export const setProjectDates = (
	slug: string,
	shoot: string | null = null,
	publish: string | null = null,
): Promise<null> => unwrap(commands.setProjectDates(slug, shoot, publish));

/** Reveal a project folder (or a D6 subfolder) in Finder. */
export const revealProjectFolder = (
	slug: string,
	subfolder: string | null = null,
): Promise<null> => unwrap(commands.revealProjectFolder(slug, subfolder));

/**
 * Move a project's folder to the macOS Trash and drop its row. Reversible:
 * Finder's "Put Back" restores the folder, and the next reconcile re-adds it.
 */
export const trashProject = (slug: string): Promise<null> =>
	unwrap(commands.trashProject(slug));
