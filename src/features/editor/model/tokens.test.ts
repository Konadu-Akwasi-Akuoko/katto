import { describe, expect, it } from "vitest";
import {
	buildTokenSpans,
	groupParagraphs,
	tokenAtTime,
} from "@/features/editor/model/tokens";
import { fixtureWords } from "@/test/fixtures/editor";

describe("buildTokenSpans", () => {
	it("builds spans preserving indices", () => {
		const spans = buildTokenSpans(fixtureWords);
		expect(spans[2]).toMatchObject({ index: 2, text: "today", kind: "word" });
	});

	it("carries speaker ids on words and null elsewhere", () => {
		const spans = buildTokenSpans(fixtureWords);
		expect(spans[0]?.speakerId).toBe("speaker_0");
		expect(spans[1]?.speakerId).toBeNull();
	});
});

describe("groupParagraphs", () => {
	it("breaks paragraphs on >1.5s gaps", () => {
		const paras = groupParagraphs(buildTokenSpans(fixtureWords));
		expect(paras).toHaveLength(2);
		expect(paras[1]?.tokens[0]?.text).toBe("Next");
	});

	it("breaks on speaker change", () => {
		const words = [
			...fixtureWords.slice(0, 3),
			{
				type: "word" as const,
				text: "Hi",
				start: 1.0,
				end: 1.2,
				logprob: -0.1,
				speaker_id: "speaker_1",
			},
		];
		const paras = groupParagraphs(buildTokenSpans(words));
		expect(paras).toHaveLength(2);
		expect(paras[1]?.tokens[0]?.text).toBe("Hi");
	});

	it("empty input yields no paragraphs", () => {
		expect(groupParagraphs([])).toHaveLength(0);
	});
});

describe("tokenAtTime", () => {
	it("finds containing token and clamps to next", () => {
		const spans = buildTokenSpans(fixtureWords);
		expect(tokenAtTime(spans, 0.5)?.text).toBe("today");
		expect(tokenAtTime(spans, 1.5)?.text).toBe("Next"); // in the gap -> next token
		expect(tokenAtTime(spans, 99)).toBeNull();
	});

	it("time before the first token resolves to it", () => {
		const spans = buildTokenSpans(fixtureWords);
		expect(tokenAtTime(spans, 0)?.text).toBe("So");
	});
});
