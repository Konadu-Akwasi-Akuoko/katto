import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTransport } from "@/features/editor/use-transport";
import type { VideoPaneHandle } from "@/features/editor/video-pane";
import type { Rational } from "@/lib/ipc/editor";

const PAL: Rational = { num: 25, den: 1 };

function makeVideo(initialTime = 0): VideoPaneHandle {
	let t = initialTime;
	let paused = true;
	let rate = 1;
	return {
		seek: vi.fn((v: number) => {
			t = v;
		}),
		getCurrentTime: vi.fn(() => t),
		play: vi.fn(() => {
			paused = false;
		}),
		pause: vi.fn(() => {
			paused = true;
		}),
		isPaused: vi.fn(() => paused),
		setRate: vi.fn((v: number) => {
			rate = v;
		}),
		getRate: vi.fn(() => rate),
	};
}

function mount(
	video: VideoPaneHandle,
	cuts: Array<{ start: number; end: number }> = [],
) {
	const videoRef = { current: video };
	return renderHook(() =>
		useTransport({ videoRef, getCutRanges: () => cuts, fps: PAL }),
	);
}

beforeEach(() => {
	vi.useFakeTimers();
});
afterEach(() => {
	vi.useRealTimers();
});

describe("useTransport", () => {
	it("L plays at 1x and a second L stacks to 2x, capped", () => {
		const video = makeVideo();
		const { result } = mount(video);
		act(() => result.current.dispatch({ kind: "play-forward" }));
		expect(video.play).toHaveBeenCalledOnce();
		expect(video.setRate).toHaveBeenLastCalledWith(1);
		act(() => result.current.dispatch({ kind: "play-forward" }));
		expect(video.setRate).toHaveBeenLastCalledWith(2);
		act(() => result.current.dispatch({ kind: "play-forward" }));
		expect(video.setRate).toHaveBeenLastCalledWith(2); // cap, not 4x
	});

	it("Space toggles play/pause and K stops at 1x", () => {
		const video = makeVideo();
		const { result } = mount(video);
		act(() => result.current.dispatch({ kind: "toggle-play" }));
		expect(video.play).toHaveBeenCalledOnce();
		expect(result.current.playing).toBe(true);
		act(() => result.current.dispatch({ kind: "toggle-play" }));
		expect(video.pause).toHaveBeenCalledOnce();
		act(() => result.current.dispatch({ kind: "play-forward" }));
		act(() => result.current.dispatch({ kind: "stop" }));
		expect(video.setRate).toHaveBeenLastCalledWith(1);
		expect(result.current.playing).toBe(false);
	});

	it("J shuttles backward on a 250ms interval and stops on play or unmount", () => {
		const video = makeVideo(10);
		const { result, unmount } = mount(video);
		act(() => result.current.dispatch({ kind: "shuttle-back" }));
		expect(video.pause).toHaveBeenCalled();
		act(() => vi.advanceTimersByTime(250));
		expect(video.seek).toHaveBeenLastCalledWith(9.75);
		act(() => vi.advanceTimersByTime(250));
		expect(video.seek).toHaveBeenLastCalledWith(9.5);
		act(() => result.current.dispatch({ kind: "toggle-play" }));
		const seeks = vi.mocked(video.seek).mock.calls.length;
		act(() => vi.advanceTimersByTime(1000));
		expect(video.seek).toHaveBeenCalledTimes(seeks); // interval cleared
		act(() => result.current.dispatch({ kind: "shuttle-back" }));
		unmount();
		act(() => vi.advanceTimersByTime(1000));
		expect(video.seek).toHaveBeenCalledTimes(seeks); // cleanup on unmount
	});

	it("backward frame-steps land just before a cut, never inside it", () => {
		const video = makeVideo(2.02);
		const { result } = mount(video, [{ start: 1, end: 2 }]);
		// 2.02 - 1/25 = 1.98, inside the cut: guard lands before its start.
		act(() => result.current.dispatch({ kind: "step-frames", frames: -1 }));
		const target = vi.mocked(video.seek).mock.lastCall?.[0];
		expect(target).toBeCloseTo(0.999, 6);
	});

	it("show-original disables the backward guard", () => {
		const video = makeVideo(2.02);
		const { result } = mount(video, [{ start: 1, end: 2 }]);
		act(() => result.current.dispatch({ kind: "toggle-original" }));
		act(() => result.current.dispatch({ kind: "step-frames", frames: -1 }));
		const target = vi.mocked(video.seek).mock.lastCall?.[0];
		expect(target).toBeCloseTo(1.98, 6);
	});

	it("shuttle ticks respect the backward guard", () => {
		const video = makeVideo(2.1);
		const { result } = mount(video, [{ start: 1, end: 2 }]);
		act(() => result.current.dispatch({ kind: "shuttle-back" }));
		act(() => vi.advanceTimersByTime(250));
		// 2.1 - 0.25 = 1.85, inside the cut: land before it.
		const target = vi.mocked(video.seek).mock.lastCall?.[0];
		expect(target).toBeCloseTo(0.999, 6);
	});
});
