import type { TemporalState } from "zundo";
import { temporal } from "zundo";
import type { StoreApi } from "zustand/vanilla";
import { createStore } from "zustand/vanilla";

import {
	addManualCut,
	adjustBaseBoundary,
	adjustManualBoundary,
	applyDiscretionary,
	convertDiscretionaryDrag,
	removeManualCut,
	toggleCut,
} from "@/features/editor/model/cut-ops";
import type { TokenSpan } from "@/features/editor/model/tokens";
import type {
	EditorDocument,
	EditorHistory,
	Range,
} from "@/features/editor/model/wire";
import { HISTORY_LIMIT } from "@/features/editor/model/wire";
import type { Cuts } from "@/lib/ipc/pipeline";

export type DragTarget =
	| { kind: "base"; cutIndex: number }
	| { kind: "manual"; index: number }
	| { kind: "disc"; index: number };

export type EditorState = EditorDocument & {
	toggleCut(cutIndex: number): void;
	applyDiscretionary(index: number): void;
	addManualCut(range: Range): void;
	removeManualCut(index: number): void;
	/** Interim drag tick — history is paused between beginDrag/commitDrag. */
	dragBoundary(
		target: DragTarget,
		edge: "start" | "end",
		newTime: number,
	): void;
	beginDrag(): void;
	commitDrag(): void;
	cancelDrag(): void;
	/** Relocation / external replacement — a normal tracked set. */
	replaceDocument(doc: EditorDocument): void;
};

export type TemporalStore = StoreApi<TemporalState<EditorDocument>>;
export type EditorStore = StoreApi<EditorState> & { temporal: TemporalStore };

/** Shared region-key -> drag-target mapping (timeline canvas + waveform). */
export function keyToDragTarget(key: string): DragTarget | null {
	const match = key.match(/^(base|manual|disc)-(\d+)$/);
	if (!match || match[2] === undefined) return null;
	const index = Number.parseInt(match[2], 10);
	if (match[1] === "base") return { kind: "base", cutIndex: index };
	if (match[1] === "manual") return { kind: "manual", index };
	return { kind: "disc", index };
}

/** The partialize projection: only document state enters history. */
export function documentOf(s: EditorDocument): EditorDocument {
	return {
		toggledOff: s.toggledOff,
		appliedDiscretionary: s.appliedDiscretionary,
		manualCuts: s.manualCuts,
		boundaryAdjustments: s.boundaryAdjustments,
	};
}

/** The temporal stacks as plain document arrays (for wire serialization). */
export function historyOf(store: EditorStore): EditorHistory {
	const t = store.temporal.getState();
	return {
		past: t.pastStates.map((d) => documentOf(d as EditorDocument)),
		future: t.futureStates.map((d) => documentOf(d as EditorDocument)),
	};
}

function deepEqualDoc(a: EditorDocument, b: EditorDocument): boolean {
	return JSON.stringify(documentOf(a)) === JSON.stringify(documentOf(b));
}

/**
 * One store per opened bundle; history seeded from edits.json at creation
 * (zundo's documented init-time pastStates/futureStates).
 *
 * Drag coalescing (the zundo caveat): zundo pushes the PREVIOUS state on each
 * tracked set, so commitDrag restores the pre-drag document while still
 * paused, resumes, then sets the final document — exactly one history entry
 * whose past state is pre-drag.
 */
export function createEditorStore(init: {
	document: EditorDocument;
	history: EditorHistory;
	cuts: Cuts;
	tokens: TokenSpan[];
}): EditorStore {
	let preDrag: EditorDocument | null = null;
	/** Set once a disc drag converts, so later ticks adjust that manual cut. */
	let discDragManualIndex: number | null = null;

	const store = createStore<EditorState>()(
		temporal(
			(set, get) => ({
				...init.document,
				toggleCut: (cutIndex) => set(toggleCut(documentOf(get()), cutIndex)),
				applyDiscretionary: (index) =>
					set(applyDiscretionary(documentOf(get()), index)),
				addManualCut: (range) =>
					set(addManualCut(documentOf(get()), range, init.tokens)),
				removeManualCut: (index) =>
					set(removeManualCut(documentOf(get()), index)),
				dragBoundary: (target, edge, newTime) => {
					const doc = documentOf(get());
					if (target.kind === "base") {
						set(adjustBaseBoundary(doc, target.cutIndex, edge, newTime));
						return;
					}
					if (target.kind === "manual") {
						set(adjustManualBoundary(doc, target.index, edge, newTime));
						return;
					}
					if (discDragManualIndex !== null) {
						set(adjustManualBoundary(doc, discDragManualIndex, edge, newTime));
						return;
					}
					discDragManualIndex = doc.manualCuts.length;
					set(
						convertDiscretionaryDrag(
							doc,
							init.cuts,
							target.index,
							edge,
							newTime,
						),
					);
				},
				beginDrag: () => {
					preDrag = documentOf(get());
					discDragManualIndex = null;
					store.temporal.getState().pause();
				},
				commitDrag: () => {
					if (preDrag === null) return;
					const final = documentOf(get());
					set(preDrag); // still paused: no history for the restore
					store.temporal.getState().resume();
					set(final); // one tracked set whose past state is preDrag
					preDrag = null;
					discDragManualIndex = null;
				},
				cancelDrag: () => {
					if (preDrag === null) return;
					set(preDrag);
					store.temporal.getState().resume();
					preDrag = null;
					discDragManualIndex = null;
				},
				replaceDocument: (doc) => set(documentOf(doc)),
			}),
			{
				limit: HISTORY_LIMIT,
				partialize: (state) => documentOf(state),
				equality: (past, current) => deepEqualDoc(past, current),
				pastStates: init.history.past,
				futureStates: init.history.future,
			},
		),
	);
	return store;
}
