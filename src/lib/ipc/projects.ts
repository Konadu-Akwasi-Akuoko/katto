import type { InvalidManifest, ReconcileReport } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { InvalidManifest, ReconcileReport };

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
