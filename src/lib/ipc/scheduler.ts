import type { ScheduledJob } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { ScheduledJob };

export const schedulerKeys = {
	all: ["scheduler"] as const,
};

/** Every scheduled job row (nightly curation, for now). */
export const getSchedulerState = (): Promise<ScheduledJob[]> =>
	unwrap(commands.getSchedulerState());

/** Run one scheduled job immediately (Settings / palette "Run now"). */
export const runScheduledJobNow = (name: string): Promise<null> =>
	unwrap(commands.runScheduledJobNow(name));

/** Update a job's daily time and enabled flag (catch-up stays fixed). */
export const setScheduledJob = (
	name: string,
	hour: number,
	minute: number,
	enabled: boolean,
): Promise<null> =>
	unwrap(commands.setScheduledJob(name, hour, minute, enabled));
