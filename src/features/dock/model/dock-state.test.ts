import { describe, expect, it } from "vitest";
import type { SessionInfo } from "@/lib/ipc/sessions";
import { deriveDockIconState, tabNote } from "./dock-state";

const base: Omit<SessionInfo, "state"> = {
	id: "s1",
	label: "cut plan: a.mp4",
	cwd: "/x",
	started_at: "2026-07-22 08:00:00",
	idle_since_secs: null,
};
const with_ = (state: SessionInfo["state"], id = "s1"): SessionInfo => ({
	...base,
	id,
	state,
});

describe("deriveDockIconState", () => {
	it("prefers needs-input over running", () => {
		expect(
			deriveDockIconState([
				with_({ kind: "running" }, "a"),
				with_({ kind: "needs_input" }, "b"),
			]),
		).toBe("needs-input");
	});
	it("running when any session runs", () => {
		expect(
			deriveDockIconState([
				with_({ kind: "idle" }, "a"),
				with_({ kind: "running" }, "b"),
			]),
		).toBe("running");
	});
	it("idle when empty or only terminal states", () => {
		expect(deriveDockIconState([])).toBe("idle");
		expect(deriveDockIconState([with_({ kind: "failed", error: "x" })])).toBe(
			"idle",
		);
	});
});

describe("tabNote", () => {
	it("notes idle reaping", () => {
		expect(tabNote(with_({ kind: "closed", reason: "idle_reaped" }))).toBe(
			"closed after idle",
		);
	});
	it("surfaces failure error", () => {
		expect(
			tabNote(with_({ kind: "failed", error: "exited with status 3" })),
		).toBe("exited with status 3");
	});
	it("is null for live sessions", () => {
		expect(tabNote(with_({ kind: "running" }))).toBeNull();
	});
});
