import type { TokenSpan } from "@/features/editor/model/tokens";
import type { Range } from "@/features/editor/model/wire";

/**
 * Map a DOM selection's [anchorTokenIndex, focusTokenIndex] to a time range
 * spanning those tokens (order-normalized); null when either index is out of
 * range.
 */
export function tokenRangeToTime(
	tokens: TokenSpan[],
	a: number,
	b: number,
): Range | null {
	const [lo, hi] = a <= b ? [a, b] : [b, a];
	const first = tokens[lo];
	const last = tokens[hi];
	if (first === undefined || last === undefined) return null;
	return { start: first.start, end: last.end };
}
