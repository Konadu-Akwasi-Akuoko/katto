import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

/** Resolve is installed (any edition) — gates the export dialog's button. */
export const resolveAvailable = (): Promise<boolean> =>
	unwrap(commands.resolveAvailable());

/**
 * Create a Resolve project from an exported timeline (newest version when
 * omitted). Requires Resolve Studio running with external scripting on;
 * typed errors carry the exact remedy.
 */
export const openInResolve = (
	slug: string,
	timelineVersion?: number,
): Promise<null> =>
	unwrap(commands.openInResolve(slug, timelineVersion ?? null));
