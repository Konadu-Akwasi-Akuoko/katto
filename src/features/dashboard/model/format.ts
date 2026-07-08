import type { Event } from "@/lib/ipc/events";

const TITLES: Record<string, string> = {
	app_started: "katto started",
	onboarding_completed: "Onboarding completed",
	key_stored: "API key saved",
	autostart_changed: "Launch at login changed",
	drive_disconnected: "Studio drive disconnected",
	drive_reconnected: "Studio drive reconnected",
};

function jobPayload(payloadJson: string | null): {
	label?: string;
	error?: string | null;
} {
	if (payloadJson === null) return {};
	try {
		const parsed: unknown = JSON.parse(payloadJson);
		if (typeof parsed !== "object" || parsed === null) return {};
		const record = parsed as Record<string, unknown>;
		return {
			label: typeof record.label === "string" ? record.label : undefined,
			error: typeof record.error === "string" ? record.error : null,
		};
	} catch {
		return {};
	}
}

/** One human-readable feed line for an activity-log event. */
export function eventLine(event: Pick<Event, "kind" | "payload_json">): string {
	if (event.kind === "job_done") {
		const { label } = jobPayload(event.payload_json);
		return `${label ?? "Job"} finished`;
	}
	if (event.kind === "job_failed") {
		const { label, error } = jobPayload(event.payload_json);
		const subject = `${label ?? "Job"} failed`;
		return error ? `${subject} — ${error}` : subject;
	}
	const known = TITLES[event.kind];
	if (known !== undefined) return known;
	const words = event.kind.replace(/_/g, " ");
	return words.charAt(0).toUpperCase() + words.slice(1);
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Compact age for feed rows: "just now", "45m ago", "9h ago", then "Jul 1". */
export function relativeTime(ts: string, now: Date): string {
	const then = new Date(ts);
	if (Number.isNaN(then.getTime())) return ts;
	const age = now.getTime() - then.getTime();
	if (age < MINUTE) return "just now";
	if (age < HOUR) return `${Math.floor(age / MINUTE)}m ago`;
	if (age < DAY) return `${Math.floor(age / HOUR)}h ago`;
	return then.toLocaleDateString("en-US", {
		month: "short",
		day: "numeric",
		timeZone: "UTC",
	});
}
