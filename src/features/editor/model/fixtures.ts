import type { Cuts, Transcript, WordEntry } from "@/lib/ipc/bindings.gen";

/** Four tokens with a 2s gap before the last — shared editor test data. */
export const fixtureWords: WordEntry[] = [
	{
		type: "word",
		text: "So",
		start: 0.12,
		end: 0.34,
		logprob: -0.2,
		speaker_id: "speaker_0",
	},
	{ type: "spacing", text: " ", start: 0.34, end: 0.47 },
	{
		type: "word",
		text: "today",
		start: 0.47,
		end: 0.9,
		logprob: -0.1,
		speaker_id: "speaker_0",
	},
	{
		type: "word",
		text: "Next",
		start: 2.9,
		end: 3.2,
		logprob: -0.1,
		speaker_id: "speaker_0",
	},
];

export const fixtureTranscript: Transcript = {
	audio_duration_secs: 10,
	language_code: "en",
	language_probability: 0.99,
	text: "So today Next",
	words: fixtureWords,
};

export const fixtureCuts: Cuts = {
	source_duration_secs: 10,
	cuts: [{ start: 0.12, end: 0.47, reason: "filler", excerpt: "So " }],
	discretionary: [
		{
			start: 0.47,
			end: 0.9,
			reason: "other",
			excerpt: "today",
			note: "n",
			confidence: "medium",
		},
	],
	flags: [
		{
			start: 2.9,
			end: 3.2,
			reason: "low_confidence",
			excerpt: "Next",
			logprob: -7.8,
		},
	],
	total_cut_secs: 0.35,
};
