import type { ReactNode } from "react";
import type { TokenOverlay } from "@/features/editor/model/overlay";
import { classifyTokens } from "@/features/editor/model/overlay";
import type { TokenSpan } from "@/features/editor/model/tokens";
import {
	buildTokenSpans,
	groupParagraphs,
} from "@/features/editor/model/tokens";
import type { Cuts, Transcript } from "@/lib/ipc/pipeline";

/** m:ss.s paragraph gutter timecode. */
function timecode(seconds: number): string {
	const m = Math.floor(seconds / 60);
	const s = seconds - m * 60;
	return `${m}:${s.toFixed(1).padStart(4, "0")}`;
}

function Token({
	span,
	overlay,
	note,
	onSeek,
}: {
	span: TokenSpan;
	overlay: TokenOverlay;
	note: string | null;
	onSeek: (seconds: number) => void;
}) {
	let inner: ReactNode;
	if (overlay?.kind === "cut") {
		inner = <del className="text-fg-faint line-through">{span.text}</del>;
	} else if (overlay?.kind === "discretionary") {
		inner = (
			<span className="underline decoration-warn decoration-dotted underline-offset-4">
				{span.text}
			</span>
		);
	} else if (overlay?.kind === "flag") {
		inner = (
			<mark className="bg-warn/20 text-fg" style={{ backgroundImage: "none" }}>
				{span.text}
			</mark>
		);
	} else if (span.kind === "audio_event") {
		inner = <span className="text-fg-muted italic">{span.text}</span>;
	} else {
		inner = span.text;
	}
	return (
		<button
			type="button"
			className="inline cursor-default whitespace-pre-wrap text-left"
			title={note ?? undefined}
			onClick={() => onSeek(span.start)}
		>
			{inner}
		</button>
	);
}

/**
 * The read-only transcript surface: paragraphs with mono gutter timecodes,
 * cuts gray-struck, discretionary amber-dotted (note on hover), flags
 * highlighted. Every token click seeks the video; nothing edits.
 */
export function TranscriptPane({
	transcript,
	cuts,
	onSeek,
}: {
	transcript: Transcript;
	cuts: Cuts | null;
	onSeek: (seconds: number) => void;
}) {
	const spans = buildTokenSpans(transcript.words);
	const overlays: TokenOverlay[] = cuts
		? classifyTokens(spans, cuts)
		: spans.map(() => null);
	const paragraphs = groupParagraphs(spans);
	let lastSpeaker: string | null = null;

	return (
		<div className="flex max-w-[65ch] flex-col gap-4 text-[15px] leading-[1.7]">
			{paragraphs.map((para) => {
				const first = para.tokens[0];
				if (first === undefined) return null;
				const speaker = first.speakerId;
				const speakerChanged = speaker !== null && speaker !== lastSpeaker;
				if (speaker !== null) lastSpeaker = speaker;
				return (
					<div key={first.index} className="flex gap-3">
						<span className="w-14 shrink-0 pt-0.5 text-right font-mono text-xs tabular-nums text-fg-faint">
							{timecode(first.start)}
						</span>
						<p className="flex-1">
							{speakerChanged && (
								<span className="mr-2 text-sm text-fg-muted">{speaker}</span>
							)}
							{para.tokens.map((span) => {
								const overlay = overlays[span.index] ?? null;
								const note =
									overlay?.kind === "discretionary"
										? (cuts?.discretionary[overlay.entryIndex]?.note ?? null)
										: null;
								return (
									<Token
										key={span.index}
										span={span}
										overlay={overlay}
										note={note}
										onSeek={onSeek}
									/>
								);
							})}
						</p>
					</div>
				);
			})}
		</div>
	);
}
