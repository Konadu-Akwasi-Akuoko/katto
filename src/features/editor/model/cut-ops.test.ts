import { describe, expect, it } from "vitest";
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
import type { EditorDocument } from "@/features/editor/model/wire";
import type { Cuts } from "@/lib/ipc/pipeline";

const doc0: EditorDocument = {
	toggledOff: [],
	appliedDiscretionary: [],
	manualCuts: [],
	boundaryAdjustments: [],
};

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
	{
		index: 3,
		text: " ",
		kind: "spacing",
		start: 0.9,
		end: 1.0,
		speakerId: null,
	},
	{ index: 4, text: "c", kind: "word", start: 1.0, end: 1.4, speakerId: null },
];

describe("cut-ops", () => {
	it("toggleCut toggles membership and returns new objects", () => {
		const a = toggleCut(doc0, 2);
		expect(a.toggledOff).toEqual([2]);
		expect(toggleCut(a, 2).toggledOff).toEqual([]);
		expect(a).not.toBe(doc0);
	});

	it("applyDiscretionary toggles membership", () => {
		const a = applyDiscretionary(doc0, 0);
		expect(a.appliedDiscretionary).toEqual([0]);
		expect(applyDiscretionary(a, 0).appliedDiscretionary).toEqual([]);
	});

	it("addManualCut snaps outward into spacing and removeManualCut deletes", () => {
		const a = addManualCut(doc0, { start: 0.47, end: 0.9 }, tokens);
		expect(a.manualCuts).toEqual([{ start: 0.34, end: 1.0, note: null }]);
		expect(removeManualCut(a, 0).manualCuts).toEqual([]);
	});

	it("adjustBaseBoundary upserts by cut and edge", () => {
		const a = adjustBaseBoundary(doc0, 0, "end", 4.0);
		const b = adjustBaseBoundary(a, 0, "end", 4.2);
		expect(b.boundaryAdjustments).toEqual([
			{ cutIndex: 0, edge: "end", newTime: 4.2 },
		]);
		const c = adjustBaseBoundary(b, 0, "start", 3.5);
		expect(c.boundaryAdjustments).toHaveLength(2);
	});

	it("adjustManualBoundary rewrites one edge of one manual cut", () => {
		const a = addManualCut(doc0, { start: 2, end: 3 }, []);
		const b = adjustManualBoundary(a, 0, "end", 3.5);
		expect(b.manualCuts).toEqual([{ start: 2, end: 3.5, note: null }]);
	});

	it("convertDiscretionaryDrag un-applies and adds an adjusted manual cut", () => {
		const withDisc = applyDiscretionary(doc0, 0);
		const out = convertDiscretionaryDrag(withDisc, fixtureCuts, 0, "end", 11.8);
		expect(out.appliedDiscretionary).toEqual([]);
		expect(out.manualCuts).toEqual([{ start: 9.0, end: 11.8, note: null }]);
	});
});
