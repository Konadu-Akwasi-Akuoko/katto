import type { TokenSpan } from "@/features/editor/model/tokens";
import type { Cuts } from "@/lib/ipc/pipeline";

export type OverlayKind = "cut" | "discretionary" | "flag";

export type TokenOverlay = { kind: OverlayKind; entryIndex: number } | null;

type Entry = { start: number; end: number; entryIndex: number };

function sortedEntries(list: { start: number; end: number }[]): Entry[] {
	return list
		.map((e, entryIndex) => ({ start: e.start, end: e.end, entryIndex }))
		.sort((a, b) => a.start - b.start);
}

/**
 * Merge walk: token mids and entry spans are both time-ordered, so a single
 * forward pointer per list replaces per-token scans (O(tokens + entries), not
 * O(tokens x entries) — an hour of footage is ~10k tokens).
 */
function assign(
	overlays: TokenOverlay[],
	spans: TokenSpan[],
	entries: Entry[],
	kind: OverlayKind,
): void {
	let i = 0;
	for (let s = 0; s < spans.length; s++) {
		const span = spans[s];
		if (span === undefined || overlays[s]) continue;
		// Midpoint containment: span boundaries sit on token edges
		// (invariant 7), so a token is covered exactly when its midpoint
		// lies inside the entry.
		const mid = (span.start + span.end) / 2;
		while (i < entries.length) {
			const entry = entries[i];
			if (entry === undefined || entry.end > mid) break;
			i++;
		}
		const entry = entries[i];
		if (entry !== undefined && entry.start < mid && mid < entry.end) {
			overlays[s] = { kind, entryIndex: entry.entryIndex };
		}
	}
}

/** Per-token classification; precedence cut > discretionary > flag. */
export function classifyTokens(spans: TokenSpan[], cuts: Cuts): TokenOverlay[] {
	const overlays: TokenOverlay[] = spans.map(() => null);
	assign(overlays, spans, sortedEntries(cuts.cuts), "cut");
	assign(overlays, spans, sortedEntries(cuts.discretionary), "discretionary");
	assign(overlays, spans, sortedEntries(cuts.flags), "flag");
	return overlays;
}
