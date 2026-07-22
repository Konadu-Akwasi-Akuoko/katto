import type { WordEntry } from "@/lib/ipc/pipeline";

export type TokenSpan = {
	/** Position in transcript.words. */
	index: number;
	text: string;
	kind: "word" | "spacing" | "audio_event";
	/** Seconds — UI boundary floats. */
	start: number;
	end: number;
	speakerId: string | null;
};

export type Paragraph = { tokens: TokenSpan[] };

/** A gap wider than this between tokens starts a new paragraph. */
const PARAGRAPH_GAP_SECONDS = 1.5;

/** Flatten transcript words into spans preserving their indices. */
export function buildTokenSpans(words: WordEntry[]): TokenSpan[] {
	return words.map((w, index) => ({
		index,
		text: w.text,
		kind: w.type,
		start: w.start,
		end: w.end,
		speakerId: w.type === "word" ? (w.speaker_id ?? null) : null,
	}));
}

/** Group spans into paragraphs: break on speaker change or a >1.5s gap. */
export function groupParagraphs(spans: TokenSpan[]): Paragraph[] {
	const paragraphs: Paragraph[] = [];
	let current: TokenSpan[] = [];
	let lastEnd: number | null = null;
	let lastSpeaker: string | null = null;

	for (const span of spans) {
		const gapBreak =
			lastEnd !== null && span.start - lastEnd > PARAGRAPH_GAP_SECONDS;
		const speakerBreak =
			span.speakerId !== null &&
			lastSpeaker !== null &&
			span.speakerId !== lastSpeaker;
		if ((gapBreak || speakerBreak) && current.length > 0) {
			paragraphs.push({ tokens: current });
			current = [];
		}
		current.push(span);
		lastEnd = span.end;
		if (span.speakerId !== null) lastSpeaker = span.speakerId;
	}
	if (current.length > 0) paragraphs.push({ tokens: current });
	return paragraphs;
}
