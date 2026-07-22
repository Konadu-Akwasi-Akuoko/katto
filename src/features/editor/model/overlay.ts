import type { TokenSpan } from "@/features/editor/model/tokens";
import type { Cuts } from "@/lib/ipc/pipeline";

export type OverlayKind = "cut" | "discretionary" | "flag";

export type TokenOverlay = { kind: OverlayKind; entryIndex: number } | null;

/** Midpoint containment: span boundaries sit on token edges (invariant 7), so
 * a token is covered exactly when its midpoint lies inside the span. */
function covers(start: number, end: number, span: TokenSpan): boolean {
	const mid = (span.start + span.end) / 2;
	return start < mid && mid < end;
}

/** Per-token classification; precedence cut > discretionary > flag. */
export function classifyTokens(spans: TokenSpan[], cuts: Cuts): TokenOverlay[] {
	return spans.map((span) => {
		const cutIndex = cuts.cuts.findIndex((c) => covers(c.start, c.end, span));
		if (cutIndex !== -1) return { kind: "cut", entryIndex: cutIndex };
		const discIndex = cuts.discretionary.findIndex((d) =>
			covers(d.start, d.end, span),
		);
		if (discIndex !== -1)
			return { kind: "discretionary", entryIndex: discIndex };
		const flagIndex = cuts.flags.findIndex((f) => covers(f.start, f.end, span));
		if (flagIndex !== -1) return { kind: "flag", entryIndex: flagIndex };
		return null;
	});
}

/** Flags are seek targets: entryIndex -> flag start seconds. */
export function flagSeekTimes(cuts: Cuts): number[] {
	return cuts.flags.map((f) => f.start);
}
