import type { Job } from "@/lib/ipc/jobs";

/** Baseline running Job row for tests; spread-override per scenario. */
export const runningJobFixture: Job = {
	id: "job-1",
	kind: "smoke",
	label: "Smoke test",
	status: "running",
	progress: 0.4,
	payload_json: null,
	error: null,
	started_at: "2026-07-08T10:00:00.000Z",
	finished_at: null,
};

export const jobsFixture: Job[] = [runningJobFixture];
