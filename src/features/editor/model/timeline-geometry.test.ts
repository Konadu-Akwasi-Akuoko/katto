import { describe, expect, it } from "vitest";
import {
	clampViewport,
	hitTest,
	pxToTime,
	regionRects,
	rulerTicks,
	thumbSlots,
	timeToPx,
	zoomAround,
} from "@/features/editor/model/timeline-geometry";

const vp = { pxPerSec: 50, scrollSec: 10, widthPx: 500 };

describe("timeline geometry", () => {
	it("time<->px round-trips through the viewport", () => {
		expect(timeToPx(12, vp)).toBe(100);
		expect(pxToTime(100, vp)).toBe(12);
	});

	it("clampViewport keeps scroll inside the media", () => {
		const clamped = clampViewport({ ...vp, scrollSec: 900 }, 600);
		expect(clamped.scrollSec).toBeCloseTo(600 - 500 / 50, 6);
		expect(clampViewport({ ...vp, scrollSec: -4 }, 600).scrollSec).toBe(0);
	});

	it("zoomAround keeps the anchor time stationary", () => {
		const z = zoomAround(vp, 250, 2, 600);
		expect(pxToTime(250, z)).toBeCloseTo(pxToTime(250, vp), 6);
		expect(z.pxPerSec).toBe(100);
	});

	it("rulerTicks picks a step keeping labels >=70px apart", () => {
		const ticks = rulerTicks(vp, 600); // 50px/s -> 2s step (100px)
		const majors = ticks.filter((t) => t.major).map((t) => t.t);
		expect(majors.slice(0, 2)).toEqual([10, 12]);
	});

	it("regionRects projects keyed ranges and discretionary outlines", () => {
		const rects = regionRects(
			[{ key: "base-0", start: 11, end: 12 }],
			[{ start: 13, end: 14 }],
			vp,
		);
		expect(rects).toEqual([
			{ key: "base-0", startPx: 50, endPx: 100, kind: "cut" },
			{ key: "disc-0", startPx: 150, endPx: 200, kind: "discretionary" },
		]);
	});

	it("hitTest prefers edges within 4px, then bodies, then empty", () => {
		const rects = [
			{ key: "base-0", startPx: 100, endPx: 160, kind: "cut" as const },
		];
		expect(hitTest(103, rects, vp)).toEqual({
			kind: "edge",
			key: "base-0",
			edge: "start",
		});
		expect(hitTest(130, rects, vp)).toEqual({ kind: "region", key: "base-0" });
		expect(hitTest(300, rects, vp)).toEqual({
			kind: "empty",
			t: pxToTime(300, vp),
		});
	});

	it("thumbSlots maps 2s cadence to 1-based %05d indices", () => {
		const slots = thumbSlots({ pxPerSec: 50, scrollSec: 0, widthPx: 300 }, 600);
		expect(slots[0]).toEqual({ index: 1, xPx: 0, wPx: 100 });
		expect(slots[1]?.index).toBe(2);
	});
});
