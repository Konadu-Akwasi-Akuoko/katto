import type { EditorDocument, Range } from "@/features/editor/model/wire";
import type { Cuts } from "@/lib/ipc/pipeline";

/** Playback does not resume within this distance of a cut's end (matches the
 * audio-editor timeupdate pattern). */
const BOUNDARY_GUARD_SECONDS = 0.005;

/** Where kept-only playback lands after skipping a cut. */
const SEEK_PAST_MARGIN_SECONDS = 0.001;

export type KeyedRange = Range & { key: string };

/**
 * Mirror of the engine merge in seconds: base minus toggledOff with boundary
 * adjustments applied (inverted spans dropped), plus applied discretionary,
 * plus manual cuts, sorted by (start, end).
 */
export function effectiveCutRanges(
	cuts: Cuts,
	doc: EditorDocument,
): KeyedRange[] {
	const out: KeyedRange[] = [];
	cuts.cuts.forEach((cut, index) => {
		if (doc.toggledOff.includes(index)) return;
		let { start, end } = cut;
		for (const adj of doc.boundaryAdjustments) {
			if (adj.cutIndex !== index) continue;
			if (adj.edge === "start") start = adj.newTime;
			else end = adj.newTime;
		}
		if (end <= start) return;
		out.push({ key: `base-${index}`, start, end });
	});
	for (const index of doc.appliedDiscretionary) {
		const disc = cuts.discretionary[index];
		if (disc === undefined) continue;
		out.push({ key: `disc-${index}`, start: disc.start, end: disc.end });
	}
	doc.manualCuts.forEach((cut, index) => {
		if (cut.end <= cut.start) return;
		out.push({ key: `manual-${index}`, start: cut.start, end: cut.end });
	});
	out.sort((a, b) => a.start - b.start || a.end - b.end);
	return out;
}

/** Sort + merge overlapping/touching ranges; drop empty ones. */
export function coalesceRanges(ranges: Range[]): Range[] {
	const sorted = ranges
		.filter((r) => r.end > r.start)
		.map((r) => ({ start: r.start, end: r.end }))
		.sort((a, b) => a.start - b.start || a.end - b.end);
	const out: Range[] = [];
	for (const range of sorted) {
		const prev = out[out.length - 1];
		if (prev !== undefined && range.start <= prev.end) {
			prev.end = Math.max(prev.end, range.end);
		} else {
			out.push(range);
		}
	}
	return out;
}

/** Complement of the (coalesced) cut ranges over [0, duration]. */
export function keptRanges(cutRanges: Range[], duration: number): Range[] {
	const merged = coalesceRanges(cutRanges);
	const out: Range[] = [];
	let cursor = 0;
	for (const cut of merged) {
		const start = Math.max(0, cut.start);
		if (start > cursor) out.push({ start: cursor, end: start });
		cursor = Math.max(cursor, Math.min(cut.end, duration));
	}
	if (duration > cursor) out.push({ start: cursor, end: duration });
	return out;
}

/** Total kept seconds. */
export function keptDuration(cutRanges: Range[], duration: number): number {
	return keptRanges(cutRanges, duration).reduce(
		(sum, r) => sum + (r.end - r.start),
		0,
	);
}

/**
 * Kept-only playback: if `t` is inside a cut (with the 5ms boundary guard),
 * the seek target just past it; else null.
 */
export function seekPastCut(t: number, coalesced: Range[]): number | null {
	for (const cut of coalesced) {
		if (t >= cut.start && t < cut.end - BOUNDARY_GUARD_SECONDS) {
			return cut.end + SEEK_PAST_MARGIN_SECONDS;
		}
	}
	return null;
}

/**
 * Backward motion (frame-step back, shuttle): if `t` lands inside a cut, the
 * seek target just BEFORE it (clamped to 0); else null. Landing before the
 * cut, not after, keeps the timeupdate forward-skip from fighting reverse
 * transport across a removed span.
 */
export function seekBeforeCut(t: number, coalesced: Range[]): number | null {
	for (const cut of coalesced) {
		if (t >= cut.start && t < cut.end) {
			return Math.max(0, cut.start - SEEK_PAST_MARGIN_SECONDS);
		}
	}
	return null;
}

/** Source time -> kept-only time (subtract removed span time before t). */
export function keptTimeOf(t: number, coalesced: Range[]): number {
	let removed = 0;
	for (const cut of coalesced) {
		if (cut.end <= t) removed += cut.end - cut.start;
		else if (cut.start < t) removed += t - cut.start;
	}
	return t - removed;
}
