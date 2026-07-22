import { describe, expect, it } from "vitest";
import { tokenRangeToTime } from "@/features/editor/model/selection";
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
	{ index: 3, text: "c", kind: "word", start: 1.0, end: 1.4, speakerId: null },
];

describe("tokenRangeToTime", () => {
	it("normalizes order and spans token extents", () => {
		expect(tokenRangeToTime(tokens, 3, 1)).toEqual({ start: 0.34, end: 1.4 });
		expect(tokenRangeToTime(tokens, 2, 2)).toEqual({ start: 0.47, end: 0.9 });
	});

	it("returns null for out-of-range indices", () => {
		expect(tokenRangeToTime(tokens, 1, 9)).toBeNull();
		expect(tokenRangeToTime([], 0, 0)).toBeNull();
	});
});
