import { commands } from "@/lib/ipc/bindings.gen";
import type { Job } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { Job };

/** List jobs newest-first; when `activeOnly`, restrict to queued/running. */
export const listJobs = (activeOnly: boolean): Promise<Job[]> =>
	unwrap(commands.listJobs(activeOnly));
