import { snapOutward } from "@/features/editor/model/snap";
import type { TokenSpan } from "@/features/editor/model/tokens";
import type { EditorDocument, Range } from "@/features/editor/model/wire";
import type { Cuts } from "@/lib/ipc/pipeline";

// Every op returns a NEW document — zundo diffs object identity.

function toggleMembership(list: number[], value: number): number[] {
	return list.includes(value)
		? list.filter((v) => v !== value)
		: [...list, value];
}

/** Toggle a base cut off/on (membership in toggledOff). */
export function toggleCut(
	doc: EditorDocument,
	cutIndex: number,
): EditorDocument {
	return { ...doc, toggledOff: toggleMembership(doc.toggledOff, cutIndex) };
}

/** Apply or un-apply a discretionary candidate. */
export function applyDiscretionary(
	doc: EditorDocument,
	index: number,
): EditorDocument {
	return {
		...doc,
		appliedDiscretionary: toggleMembership(doc.appliedDiscretionary, index),
	};
}

/** Add a manual cut, outward-snapped into adjacent spacing tokens. */
export function addManualCut(
	doc: EditorDocument,
	range: Range,
	tokens: TokenSpan[],
): EditorDocument {
	const snapped = snapOutward(range, tokens);
	return {
		...doc,
		manualCuts: [...doc.manualCuts, { ...snapped, note: null }],
	};
}

/** Delete a manual cut by position. */
export function removeManualCut(
	doc: EditorDocument,
	index: number,
): EditorDocument {
	return {
		...doc,
		manualCuts: doc.manualCuts.filter((_, i) => i !== index),
	};
}

/**
 * Clamp a dragged edge so the range keeps at least one frame of width (and
 * a start never goes negative) — an inverted range would silently erase the
 * cut and persist the inversion.
 */
export function clampEdge(
	range: Range,
	edge: "start" | "end",
	newTime: number,
	frameSeconds: number,
): number {
	return edge === "start"
		? Math.max(0, Math.min(newTime, range.end - frameSeconds))
		: Math.max(newTime, range.start + frameSeconds);
}

/** Upsert a base-cut boundary adjustment by (cutIndex, edge). */
export function adjustBaseBoundary(
	doc: EditorDocument,
	cutIndex: number,
	edge: "start" | "end",
	newTime: number,
): EditorDocument {
	const rest = doc.boundaryAdjustments.filter(
		(b) => !(b.cutIndex === cutIndex && b.edge === edge),
	);
	return {
		...doc,
		boundaryAdjustments: [...rest, { cutIndex, edge, newTime }],
	};
}

/** Move one edge of a manual cut. */
export function adjustManualBoundary(
	doc: EditorDocument,
	index: number,
	edge: "start" | "end",
	newTime: number,
): EditorDocument {
	return {
		...doc,
		manualCuts: doc.manualCuts.map((m, i) =>
			i === index ? { ...m, [edge]: newTime } : m,
		),
	};
}

/**
 * Dragging an applied-discretionary edge converts it: un-apply + manual cut at
 * the adjusted range (the wire format has no discretionary boundary mechanism).
 */
export function convertDiscretionaryDrag(
	doc: EditorDocument,
	cuts: Cuts,
	index: number,
	edge: "start" | "end",
	newTime: number,
): EditorDocument {
	const disc = cuts.discretionary[index];
	if (disc === undefined) return doc;
	const range: Range = { start: disc.start, end: disc.end };
	range[edge] = newTime;
	return {
		...doc,
		appliedDiscretionary: doc.appliedDiscretionary.filter((i) => i !== index),
		manualCuts: [...doc.manualCuts, { ...range, note: null }],
	};
}
