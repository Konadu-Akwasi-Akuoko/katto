import { describe, expect, it } from "vitest";
import {
	classifyEffective,
	classifyTokens,
} from "@/features/editor/model/overlay";
import { buildTokenSpans } from "@/features/editor/model/tokens";
import type { EditorDocument } from "@/features/editor/model/wire";
import { fixtureCuts, fixtureWords } from "@/test/fixtures/editor";

const doc0: EditorDocument = {
	toggledOff: [],
	appliedDiscretionary: [],
	manualCuts: [],
	boundaryAdjustments: [],
};

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

describe("classifyEffective", () => {
	it("strikes effective cuts by key and keeps unapplied discretionary amber", () => {
		const overlays = classifyEffective(
			buildTokenSpans(fixtureWords),
			fixtureCuts,
			doc0,
		);
		expect(overlays[0]).toMatchObject({ kind: "cut", key: "base-0" });
		expect(overlays[2]).toMatchObject({ kind: "discretionary", entryIndex: 0 });
		expect(overlays[3]).toMatchObject({ kind: "flag", entryIndex: 0 });
	});

	it("an applied discretionary strikes with its canonical disc key", () => {
		const overlays = classifyEffective(
			buildTokenSpans(fixtureWords),
			fixtureCuts,
			{
				...doc0,
				appliedDiscretionary: [0],
			},
		);
		expect(overlays[2]).toMatchObject({ kind: "cut", key: "disc-0" });
	});

	it("a toggled-off base cut releases its tokens", () => {
		const overlays = classifyEffective(
			buildTokenSpans(fixtureWords),
			fixtureCuts,
			{
				...doc0,
				toggledOff: [0],
			},
		);
		expect(overlays[0]).toBeNull();
	});
});
