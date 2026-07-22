import { describe, expect, it } from "vitest";
import {
	nearestTokenBoundary,
	snapEdge,
	snapOutward,
} from "@/features/editor/model/snap";
import type { TokenSpan } from "@/features/editor/model/tokens";

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

describe("snap", () => {
	it("nearestTokenBoundary finds the closest edge within maxDelta", () => {
		expect(nearestTokenBoundary(0.45, tokens, 0.1)).toBe(0.47);
		expect(nearestTokenBoundary(5.0, tokens, 0.1)).toBeNull();
	});

	it("snapOutward expands into adjacent spacing tokens", () => {
		// selecting the middle word (0.47..0.90) expands to the spacing envelope
		expect(snapOutward({ start: 0.47, end: 0.9 }, tokens)).toEqual({
			start: 0.34,
			end: 1.0,
		});
	});

	it("snapOutward leaves a range alone with no adjacent spacing", () => {
		expect(snapOutward({ start: 5, end: 6 }, tokens)).toEqual({
			start: 5,
			end: 6,
		});
	});

	it("snapEdge honors free drag by snapping to the frame grid only", () => {
		expect(snapEdge(0.451, tokens, { num: 25, den: 1 }, false)).toBe(0.47);
		expect(snapEdge(0.451, tokens, { num: 25, den: 1 }, true)).toBeCloseTo(
			0.44,
			5,
		); // frame 11.275 -> 11 -> 0.44
	});

	it("snapEdge falls back to the frame grid away from any token", () => {
		expect(snapEdge(5.003, tokens, { num: 25, den: 1 }, false)).toBeCloseTo(
			5.0,
			5,
		);
	});
});
