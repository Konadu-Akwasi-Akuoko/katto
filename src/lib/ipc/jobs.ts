import { Channel } from "@tauri-apps/api/core";
import type { Job, JobProgress } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { Job, JobProgress };

export const jobsKeys = {
	all: ["jobs"] as const,
	active: ["jobs", "active"] as const,
};

/** List jobs newest-first; when `activeOnly`, restrict to queued/running. */
export const listJobs = (activeOnly: boolean): Promise<Job[]> =>
	unwrap(commands.listJobs(activeOnly));

/**
 * Stream one job's live progress. The backend replays the current snapshot
 * first, so late subscribers (a reopened window) start correct.
 */
export const subscribeJobProgress = (
	jobId: string,
	onProgress: (update: JobProgress) => void,
): Promise<null> => {
	const channel = new Channel<JobProgress>();
	channel.onmessage = onProgress;
	return unwrap(commands.subscribeJobProgress(jobId, channel));
};

/** Dev-only synthetic job exercising the whole jobs pipeline. */
export const devRunSmokeJob = (fail: boolean): Promise<Job> =>
	unwrap(commands.devRunSmokeJob(fail));
