import type { Range } from "@/features/editor/model/wire";

// Pure timeline math: ALL interaction logic lives here; the canvas component
// only draws and forwards pointer events.

export type Viewport = { pxPerSec: number; scrollSec: number; widthPx: number };

const MIN_PX_PER_SEC = 2;
const MAX_PX_PER_SEC = 500;

/** Edge grab zone in px. */
const EDGE_GRAB_PX = 4;

/** Thumbnail cadence (matches the engine's fps=1/2 extraction). */
const THUMB_SLOT_SECONDS = 2;

export function timeToPx(t: number, vp: Viewport): number {
	return (t - vp.scrollSec) * vp.pxPerSec;
}

export function pxToTime(x: number, vp: Viewport): number {
	return vp.scrollSec + x / vp.pxPerSec;
}

/** Clamp zoom to sane bounds and scroll into [0, duration - visible]. */
export function clampViewport(vp: Viewport, duration: number): Viewport {
	const pxPerSec = Math.min(
		MAX_PX_PER_SEC,
		Math.max(MIN_PX_PER_SEC, vp.pxPerSec),
	);
	const visible = vp.widthPx / pxPerSec;
	const maxScroll = Math.max(0, duration - visible);
	const scrollSec = Math.min(maxScroll, Math.max(0, vp.scrollSec));
	return { pxPerSec, scrollSec, widthPx: vp.widthPx };
}

/** Zoom by `factor` keeping the time under `anchorPx` stationary. */
export function zoomAround(
	vp: Viewport,
	anchorPx: number,
	factor: number,
	duration: number,
): Viewport {
	const anchorTime = pxToTime(anchorPx, vp);
	const pxPerSec = Math.min(
		MAX_PX_PER_SEC,
		Math.max(MIN_PX_PER_SEC, vp.pxPerSec * factor),
	);
	const scrollSec = anchorTime - anchorPx / pxPerSec;
	return clampViewport({ pxPerSec, scrollSec, widthPx: vp.widthPx }, duration);
}

/** Ruler ticks: major step from 1/2/5/10/30/60s so labels sit >=70px apart;
 * minors at half-steps. */
export function rulerTicks(
	vp: Viewport,
	duration: number,
): Array<{ t: number; major: boolean }> {
	const steps = [1, 2, 5, 10, 30, 60];
	const step = steps.find((s) => s * vp.pxPerSec >= 70) ?? 60;
	const minor = step / 2;
	const visibleEnd = Math.min(duration, pxToTime(vp.widthPx, vp));
	const first = Math.max(0, Math.floor(vp.scrollSec / minor) * minor);
	const ticks: Array<{ t: number; major: boolean }> = [];
	for (let t = first; t <= visibleEnd; t += minor) {
		ticks.push({ t, major: Math.abs(t / step - Math.round(t / step)) < 1e-9 });
	}
	return ticks;
}

export type RegionRect = {
	key: string;
	startPx: number;
	endPx: number;
	kind: "cut" | "discretionary";
};

/** Project effective cut ranges (dimmed absence) and unapplied discretionary
 * spans (dotted outline) into pixel rects. Unapplied spans arrive pre-keyed
 * with their canonical `disc-N` index — positional keys would collide with
 * applied discretionaries and target the wrong entry on apply. */
export function regionRects(
	ranges: Array<Range & { key: string }>,
	discretionaryUnapplied: Array<Range & { key: string }>,
	vp: Viewport,
): RegionRect[] {
	const rects: RegionRect[] = ranges.map((r) => ({
		key: r.key,
		startPx: timeToPx(r.start, vp),
		endPx: timeToPx(r.end, vp),
		kind: "cut",
	}));
	for (const r of discretionaryUnapplied) {
		rects.push({
			key: r.key,
			startPx: timeToPx(r.start, vp),
			endPx: timeToPx(r.end, vp),
			kind: "discretionary",
		});
	}
	return rects;
}

export type HitTarget =
	| { kind: "edge"; key: string; edge: "start" | "end" }
	| { kind: "region"; key: string }
	| { kind: "empty"; t: number };

/** Edge grab zone ±4px, else region body, else empty. */
export function hitTest(
	xPx: number,
	rects: RegionRect[],
	vp: Viewport,
): HitTarget {
	for (const rect of rects) {
		if (Math.abs(xPx - rect.startPx) <= EDGE_GRAB_PX) {
			return { kind: "edge", key: rect.key, edge: "start" };
		}
		if (Math.abs(xPx - rect.endPx) <= EDGE_GRAB_PX) {
			return { kind: "edge", key: rect.key, edge: "end" };
		}
	}
	for (const rect of rects) {
		if (xPx >= rect.startPx && xPx <= rect.endPx) {
			return { kind: "region", key: rect.key };
		}
	}
	return { kind: "empty", t: pxToTime(xPx, vp) };
}

/** Which 1-based %05d.jpg covers each visible 2s slot of the filmstrip. */
export function thumbSlots(
	vp: Viewport,
	duration: number,
): Array<{ index: number; xPx: number; wPx: number }> {
	const firstSlot = Math.max(0, Math.floor(vp.scrollSec / THUMB_SLOT_SECONDS));
	const visibleEnd = Math.min(duration, pxToTime(vp.widthPx, vp));
	const slots: Array<{ index: number; xPx: number; wPx: number }> = [];
	for (let slot = firstSlot; slot * THUMB_SLOT_SECONDS < visibleEnd; slot++) {
		slots.push({
			index: slot + 1,
			xPx: timeToPx(slot * THUMB_SLOT_SECONDS, vp),
			wPx: THUMB_SLOT_SECONDS * vp.pxPerSec,
		});
	}
	return slots;
}
