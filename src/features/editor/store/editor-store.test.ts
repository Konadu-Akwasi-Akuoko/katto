import { describe, expect, it } from "vitest";
import type { TokenSpan } from "@/features/editor/model/tokens";
import { fromWireEdits, toWireEdits } from "@/features/editor/model/wire";
import {
	createEditorStore,
	documentOf,
	historyOf,
} from "@/features/editor/store/editor-store";
import type { Cuts } from "@/lib/ipc/pipeline";

const fixtureCuts: Cuts = {
	source_duration_secs: 20,
	cuts: [
		{ start: 1, end: 2, reason: "filler", excerpt: "" },
		{ start: 4, end: 5, reason: "stutter", excerpt: "" },
	],
	discretionary: [
		{
			start: 9.0,
			end: 11.2,
			reason: "other",
			excerpt: "",
			note: "n",
			confidence: "medium",
		},
	],
	flags: [],
	total_cut_secs: 2,
};

const tokens: TokenSpan[] = [
	{
		index: 0,
		text: "a",
		kind: "word",
		start: 0.12,
		end: 0.34,
		speakerId: null,
	},
	{
		index: 1,
		text: " ",
		kind: "spacing",
		start: 0.34,
		end: 0.47,
		speakerId: null,
	},
	{ index: 2, text: "b", kind: "word", start: 0.47, end: 0.9, speakerId: null },
];

const PAL = { num: 25, den: 1 };

function init() {
	return {
		document: {
			toggledOff: [],
			appliedDiscretionary: [],
			manualCuts: [],
			boundaryAdjustments: [],
		},
		history: { past: [], future: [] },
		cuts: fixtureCuts,
		tokens,
	};
}

describe("editor store", () => {
	it("each discrete edit is one undo step, Cmd+Z semantics", () => {
		const store = createEditorStore(init());
		store.getState().toggleCut(0);
		store.getState().applyDiscretionary(0);
		expect(store.temporal.getState().pastStates).toHaveLength(2);
		store.temporal.getState().undo();
		expect(documentOf(store.getState()).appliedDiscretionary).toEqual([]);
		expect(documentOf(store.getState()).toggledOff).toEqual([0]);
	});

	it("a drag of many ticks is exactly one history entry with pre-drag past state", () => {
		const store = createEditorStore(init());
		store.getState().beginDrag();
		for (const t of [4.1, 4.2, 4.3, 4.4]) {
			store.getState().dragBoundary({ kind: "base", cutIndex: 0 }, "end", t);
		}
		store.getState().commitDrag();
		expect(store.temporal.getState().pastStates).toHaveLength(1);
		expect(documentOf(store.getState()).boundaryAdjustments).toEqual([
			{ cutIndex: 0, edge: "end", newTime: 4.4 },
		]);
		store.temporal.getState().undo();
		expect(documentOf(store.getState()).boundaryAdjustments).toEqual([]);
	});

	it("cancelDrag restores the pre-drag document without history", () => {
		const store = createEditorStore(init());
		store.getState().beginDrag();
		store.getState().dragBoundary({ kind: "base", cutIndex: 0 }, "end", 4.2);
		store.getState().cancelDrag();
		expect(documentOf(store.getState()).boundaryAdjustments).toEqual([]);
		expect(store.temporal.getState().pastStates).toHaveLength(0);
	});

	it("a discretionary edge drag converts once and keeps adjusting one manual cut", () => {
		const store = createEditorStore(init());
		store.getState().applyDiscretionary(0);
		store.getState().beginDrag();
		for (const t of [11.4, 11.6, 11.8]) {
			store.getState().dragBoundary({ kind: "disc", index: 0 }, "end", t);
		}
		store.getState().commitDrag();
		const doc = documentOf(store.getState());
		expect(doc.appliedDiscretionary).toEqual([]);
		expect(doc.manualCuts).toEqual([{ start: 9.0, end: 11.8, note: null }]);
	});

	it("history seeds from edits.json and survives recreation (restart simulation)", () => {
		const a = createEditorStore(init());
		a.getState().toggleCut(0);
		const wire = toWireEdits(documentOf(a.getState()), historyOf(a), PAL);
		const b = createEditorStore({ ...init(), ...fromWireEdits(wire, PAL) });
		expect(b.temporal.getState().pastStates).toHaveLength(1);
		b.temporal.getState().undo();
		expect(documentOf(b.getState()).toggledOff).toEqual([]);
	});
});
