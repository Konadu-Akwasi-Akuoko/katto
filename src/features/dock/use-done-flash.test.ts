import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/lib/ipc/sessions";
import { useDoneFlash } from "./use-done-flash";

function session(id: string, kind: "running" | "idle" | "closed"): SessionInfo {
	return {
		id,
		label: id,
		state:
			kind === "closed"
				? { kind, reason: "user_closed" }
				: kind === "idle"
					? { kind }
					: { kind },
		cwd: "/tmp",
		started_at: "2026-07-22T00:00:00Z",
		idle_since_secs: null,
	} as SessionInfo;
}

describe("useDoneFlash", () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});
	afterEach(() => {
		vi.useRealTimers();
	});

	it("flashes when a running session settles, then clears after 3s", () => {
		const { result, rerender } = renderHook(
			({ sessions }: { sessions: SessionInfo[] }) => useDoneFlash(sessions),
			{ initialProps: { sessions: [session("a", "running")] } },
		);
		expect(result.current).toBe(false);

		rerender({ sessions: [session("a", "idle")] });
		expect(result.current).toBe(true);

		act(() => {
			vi.advanceTimersByTime(3000);
		});
		expect(result.current).toBe(false);
	});

	it("still clears when an unrelated sessions change lands mid-flash", () => {
		const { result, rerender } = renderHook(
			({ sessions }: { sessions: SessionInfo[] }) => useDoneFlash(sessions),
			{ initialProps: { sessions: [session("a", "running")] } },
		);
		rerender({ sessions: [session("a", "idle")] });
		expect(result.current).toBe(true);

		// A non-settling update (new session spawned) must not strand the
		// flash lit by cancelling its timer.
		rerender({
			sessions: [session("a", "idle"), session("b", "running")],
		});
		act(() => {
			vi.advanceTimersByTime(3000);
		});
		expect(result.current).toBe(false);
	});
});
