import { effectiveCutRanges } from "@/features/editor/model/kept-ranges";
import type { TokenSpan } from "@/features/editor/model/tokens";
import type { EditorDocument } from "@/features/editor/model/wire";
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

/** Editing-mode per-token classification against the EFFECTIVE cut state. */
export type EffectiveOverlay =
	| { kind: "cut"; key: string }
	| { kind: "discretionary"; entryIndex: number }
	| { kind: "flag"; entryIndex: number }
	| null;

/**
 * Like {@link classifyTokens} but over `effectiveCutRanges(cuts, doc)`:
 * struck = any effective cut (base/applied-disc/manual, keyed for click
 * handling); amber = still-unapplied discretionary; flags unchanged.
 */
export function classifyEffective(
	spans: TokenSpan[],
	cuts: Cuts,
	doc: EditorDocument,
): EffectiveOverlay[] {
	const ranges = effectiveCutRanges(cuts, doc);
	const base: TokenOverlay[] = spans.map(() => null);
	assign(
		base,
		spans,
		ranges.map((r, entryIndex) => ({ start: r.start, end: r.end, entryIndex })),
		"cut",
	);
	const unapplied = cuts.discretionary
		.map((d, entryIndex) => ({ start: d.start, end: d.end, entryIndex }))
		.filter((d) => !doc.appliedDiscretionary.includes(d.entryIndex))
		.sort((a, b) => a.start - b.start);
	assign(base, spans, unapplied, "discretionary");
	assign(base, spans, sortedEntries(cuts.flags), "flag");
	return base.map((o): EffectiveOverlay => {
		if (o === null) return null;
		if (o.kind === "cut") {
			const key = ranges[o.entryIndex]?.key;
			return key === undefined ? null : { kind: "cut", key };
		}
		if (o.kind === "discretionary") {
			return { kind: "discretionary", entryIndex: o.entryIndex };
		}
		return { kind: "flag", entryIndex: o.entryIndex };
	});
}
