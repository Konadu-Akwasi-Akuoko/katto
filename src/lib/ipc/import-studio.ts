import type { ImportOutcome, ImportReport } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { ImportOutcome, ImportReport };

/**
 * Import the old planner's ideas from a studio.db file. Dry-run answers with
 * the mapped preview; apply spawns a job whose final report arrives over the
 * StudioImportFinished broadcast. `~/` paths expand backend-side.
 */
export const importStudioDb = (
	path: string,
	dryRun: boolean,
): Promise<ImportOutcome> => unwrap(commands.importStudioDb(path, dryRun));
