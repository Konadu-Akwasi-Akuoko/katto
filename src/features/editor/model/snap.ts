import type { TokenSpan } from "@/features/editor/model/tokens";
import type { Range } from "@/features/editor/model/wire";
import type { Rational } from "@/lib/ipc/bindings.gen";

/** How far an edge drag reaches for a token boundary before frame fallback. */
const EDGE_SNAP_DELTA_SECONDS = 0.15;

/** Two float times that differ by less than this are the same boundary. */
const BOUNDARY_EPSILON = 1e-6;

/** Nearest word-boundary time (any token's start/end) to `t`; null when none
 * within `maxDelta`. */
export function nearestTokenBoundary(
	t: number,
	tokens: TokenSpan[],
	maxDelta: number,
): number | null {
	let best: number | null = null;
	let bestDelta = maxDelta;
	for (const token of tokens) {
		for (const boundary of [token.start, token.end]) {
			const delta = Math.abs(boundary - t);
			if (delta <= bestDelta) {
				bestDelta = delta;
				best = boundary;
			}
		}
	}
	return best;
}

/** Nearest frame time of `fps` to `t`. */
function snapToFrame(t: number, fps: Rational): number {
	const frame = Math.round((t * fps.num) / fps.den);
	return (frame * fps.den) / fps.num;
}

/** Snap for region-edge drags: token boundary unless `free` (Option held),
 * then frame grid only. */
export function snapEdge(
	t: number,
	tokens: TokenSpan[],
	fps: Rational,
	free: boolean,
): number {
	if (!free) {
		const boundary = nearestTokenBoundary(t, tokens, EDGE_SNAP_DELTA_SECONDS);
		if (boundary !== null) return boundary;
	}
	return snapToFrame(t, fps);
}

/**
 * Manual-cut ranges snap OUTWARD into adjacent spacing tokens (PRD): start
 * moves left to the start of a covering/preceding spacing token, end moves
 * right to the end of a covering/following one.
 */
export function snapOutward(range: Range, tokens: TokenSpan[]): Range {
	let { start, end } = range;
	for (const token of tokens) {
		if (token.kind !== "spacing") continue;
		const coversStart =
			token.start <= start + BOUNDARY_EPSILON &&
			start <= token.end + BOUNDARY_EPSILON;
		if (coversStart) start = Math.min(start, token.start);
		const coversEnd =
			token.start <= end + BOUNDARY_EPSILON &&
			end <= token.end + BOUNDARY_EPSILON;
		if (coversEnd) end = Math.max(end, token.end);
	}
	return { start, end };
}
