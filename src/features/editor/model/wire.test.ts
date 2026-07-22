import { describe, expect, it } from "vitest";
import type { EditorDocument } from "@/features/editor/model/wire";
import {
	fromWireEdits,
	rationalToSeconds,
	secondsToRational,
	toWireEdits,
} from "@/features/editor/model/wire";
import type { Rational } from "@/lib/ipc/editor";

const NTSC: Rational = { num: 30000, den: 1001 };
const PAL: Rational = { num: 25, den: 1 };

describe("secondsToRational", () => {
	it("lands on the frame grid", () => {
		expect(secondsToRational(0.5, NTSC)).toEqual({
			num: 15 * 1001,
			den: 30000,
		}); // frame 15
		expect(secondsToRational(0, NTSC)).toEqual({ num: 0, den: 30000 });
	});

	it("round-trips through rationalToSeconds within a frame", () => {
		const r = secondsToRational(4.21, PAL);
		expect(Math.abs(rationalToSeconds(r) - 4.21)).toBeLessThan(1 / 25);
	});
});

describe("wire round-trip", () => {
	it("preserves the document and caps history at 100", () => {
		const doc: EditorDocument = {
			toggledOff: [1],
			appliedDiscretionary: [0],
			manualCuts: [{ start: 1.5, end: 2.5, note: null }],
			boundaryAdjustments: [{ cutIndex: 0, edge: "end", newTime: 4.0 }],
		};
		const history = {
			past: Array.from({ length: 130 }, () => doc),
			future: [doc],
		};
		const wire = toWireEdits(doc, history, PAL);
		expect(wire.history?.past).toHaveLength(100);
		const back = fromWireEdits(wire, PAL);
		expect(back.document).toEqual(doc);
		expect(back.history.future).toHaveLength(1);
		expect(back.history.past).toHaveLength(100);
	});

	it("fromWireEdits of null yields an empty document", () => {
		const { document, history } = fromWireEdits(null, PAL);
		expect(document.toggledOff).toEqual([]);
		expect(history.past).toEqual([]);
	});
});
