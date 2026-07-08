import { describe, expect, it } from "vitest";
import { eventLine, relativeTime } from "@/features/dashboard/model/format";

describe("eventLine", () => {
	it("maps known kinds to plain sentences", () => {
		expect(eventLine({ kind: "app_started", payload_json: null })).toBe("katto started");
		expect(eventLine({ kind: "onboarding_completed", payload_json: null })).toBe(
			"Onboarding completed",
		);
	});

	it("names the job in terminal job events", () => {
		const payload = JSON.stringify({ job_id: "j1", label: "Smoke test", error: null });
		expect(eventLine({ kind: "job_done", payload_json: payload })).toBe("Smoke test finished");
	});

	it("includes the error for failed jobs", () => {
		const payload = JSON.stringify({ job_id: "j1", label: "Smoke test", error: "disk full" });
		expect(eventLine({ kind: "job_failed", payload_json: payload })).toBe(
			"Smoke test failed — disk full",
		);
	});

	it("shows the drive path on drive events", () => {
		const payload = JSON.stringify({ path: "/Volumes/Studio" });
		expect(eventLine({ kind: "drive_disconnected", payload_json: payload })).toBe(
			"Studio drive disconnected",
		);
		expect(eventLine({ kind: "drive_reconnected", payload_json: payload })).toBe(
			"Studio drive reconnected",
		);
	});

	it("humanizes unknown kinds instead of leaking snake_case", () => {
		expect(eventLine({ kind: "thumbnail_rendered", payload_json: null })).toBe(
			"Thumbnail rendered",
		);
	});

	it("survives malformed payload json", () => {
		expect(eventLine({ kind: "job_done", payload_json: "{not json" })).toBe("Job finished");
	});
});

describe("relativeTime", () => {
	const now = new Date("2026-07-08T12:00:00.000Z");

	it("says just now under a minute", () => {
		expect(relativeTime("2026-07-08T11:59:30.000Z", now)).toBe("just now");
	});

	it("uses minutes under an hour", () => {
		expect(relativeTime("2026-07-08T11:15:00.000Z", now)).toBe("45m ago");
	});

	it("uses hours under a day", () => {
		expect(relativeTime("2026-07-08T03:00:00.000Z", now)).toBe("9h ago");
	});

	it("falls back to a date beyond a day", () => {
		expect(relativeTime("2026-07-01T12:00:00.000Z", now)).toBe("Jul 1");
	});

	it("treats unparseable timestamps as raw text", () => {
		expect(relativeTime("garbage", now)).toBe("garbage");
	});
});
