import type { Event } from "@/lib/ipc/events";

/** Newest-first activity-log page for tests; spread-override per scenario. */
export const eventsFixture: Event[] = [
	{
		id: 3,
		ts: "2026-07-08T10:15:00.000Z",
		kind: "job_done",
		project_slug: null,
		payload_json: JSON.stringify({
			job_id: "job-1",
			label: "Smoke test",
			error: null,
		}),
	},
	{
		id: 2,
		ts: "2026-07-08T09:00:00.000Z",
		kind: "drive_disconnected",
		project_slug: null,
		payload_json: JSON.stringify({ path: "/Volumes/Studio" }),
	},
	{
		id: 1,
		ts: "2026-07-08T08:00:00.000Z",
		kind: "app_started",
		project_slug: null,
		payload_json: null,
	},
];
