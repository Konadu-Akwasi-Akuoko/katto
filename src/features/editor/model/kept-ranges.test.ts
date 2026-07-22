import { describe, expect, it } from "vitest";
import {
	coalesceRanges,
	effectiveCutRanges,
	keptDuration,
	keptRanges,
	seekPastCut,
} from "@/features/editor/model/kept-ranges";
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

describe("kept-ranges", () => {
	it("effectiveCutRanges mirrors the engine merge semantics", () => {
		const doc = {
			...doc0,
			toggledOff: [0],
			manualCuts: [{ start: 7, end: 8, note: null }],
		};
		const out = effectiveCutRanges(fixtureCuts, doc);
		expect(out.map((r) => r.key)).toEqual(["base-1", "manual-0"]);
	});

	it("applies boundary adjustments and drops inverted spans", () => {
		const doc = {
			...doc0,
			boundaryAdjustments: [
				{ cutIndex: 0, edge: "end" as const, newTime: 2.4 },
				{ cutIndex: 1, edge: "end" as const, newTime: 3.9 },
			],
		};
		const out = effectiveCutRanges(fixtureCuts, doc);
		expect(out).toEqual([{ key: "base-0", start: 1, end: 2.4 }]);
	});

	it("applied discretionary joins with its own key", () => {
		const doc = { ...doc0, appliedDiscretionary: [0] };
		const out = effectiveCutRanges(fixtureCuts, doc);
		expect(out.map((r) => r.key)).toEqual(["base-0", "base-1", "disc-0"]);
	});

	it("coalesceRanges merges touching and overlapping spans", () => {
		expect(
			coalesceRanges([
				{ start: 2, end: 3 },
				{ start: 1, end: 2 },
				{ start: 2.5, end: 2.6 },
			]),
		).toEqual([{ start: 1, end: 3 }]);
	});

	it("keptRanges complements and seekPastCut jumps with the 5ms guard", () => {
		const cuts = [{ start: 1, end: 2 }];
		expect(keptRanges(cuts, 5)).toEqual([
			{ start: 0, end: 1 },
			{ start: 2, end: 5 },
		]);
		expect(keptDuration(cuts, 5)).toBeCloseTo(4, 9);
		expect(seekPastCut(1.5, cuts)).toBeCloseTo(2.001, 3);
		expect(seekPastCut(1.997, cuts)).toBeNull(); // inside the 5ms boundary guard
		expect(seekPastCut(0.5, cuts)).toBeNull();
	});
});
