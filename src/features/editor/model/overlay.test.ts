import { describe, expect, it } from "vitest";
import { classifyTokens } from "@/features/editor/model/overlay";
import { buildTokenSpans } from "@/features/editor/model/tokens";
import { fixtureCuts, fixtureWords } from "@/test/fixtures/editor";

describe("classifyTokens", () => {
	it("classifies each covered token once with precedence", () => {
		const overlays = classifyTokens(buildTokenSpans(fixtureWords), fixtureCuts);
		expect(overlays[0]).toMatchObject({ kind: "cut", entryIndex: 0 }); // "So"
		expect(overlays[1]).toMatchObject({ kind: "cut", entryIndex: 0 }); // spacing inside cut
		expect(overlays[2]).toMatchObject({ kind: "discretionary", entryIndex: 0 });
		expect(overlays[3]).toMatchObject({ kind: "flag", entryIndex: 0 });
	});

	it("uncovered tokens are null", () => {
		const overlays = classifyTokens(buildTokenSpans(fixtureWords), {
			...fixtureCuts,
			cuts: [],
			discretionary: [],
			flags: [],
		});
		expect(overlays.every((o) => o === null)).toBe(true);
	});

	it("cut wins over an overlapping flag", () => {
		const overlays = classifyTokens(buildTokenSpans(fixtureWords), {
			...fixtureCuts,
			discretionary: [],
			flags: [
				{
					start: 0.12,
					end: 0.47,
					reason: "low_confidence",
					excerpt: "So",
					logprob: -7.8,
				},
			],
		});
		expect(overlays[0]).toMatchObject({ kind: "cut" });
	});
});
