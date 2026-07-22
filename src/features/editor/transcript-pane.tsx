import type { ReactNode } from "react";
import { useEffect, useMemo, useRef } from "react";
import type { EffectiveOverlay } from "@/features/editor/model/overlay";
import { classifyEffective } from "@/features/editor/model/overlay";
import { tokenRangeToTime } from "@/features/editor/model/selection";
import type { TokenSpan } from "@/features/editor/model/tokens";
import {
	buildTokenSpans,
	groupParagraphs,
} from "@/features/editor/model/tokens";
import type { EditorDocument, Range } from "@/features/editor/model/wire";
import { isEditableTarget } from "@/features/editor/transport";
import type { Cuts, Transcript } from "@/lib/ipc/pipeline";

const EMPTY_DOCUMENT: EditorDocument = {
	toggledOff: [],
	appliedDiscretionary: [],
	manualCuts: [],
	boundaryAdjustments: [],
};

/** m:ss.s paragraph gutter timecode. */
function timecode(seconds: number): string {
	const m = Math.floor(seconds / 60);
	const s = seconds - m * 60;
	return `${m}:${s.toFixed(1).padStart(4, "0")}`;
}

/** The current DOM selection projected onto token indices -> a time range. */
function selectionTimeRange(tokens: TokenSpan[]): Range | null {
	const selection = window.getSelection();
	if (!selection || selection.isCollapsed) return null;
	const indexOf = (node: Node | null): number | null => {
		const el =
			node instanceof HTMLElement ? node : (node?.parentElement ?? null);
		const host = el?.closest("[data-token-index]");
		const raw = host?.getAttribute("data-token-index");
		if (raw == null) return null;
		const parsed = Number.parseInt(raw, 10);
		return Number.isNaN(parsed) ? null : parsed;
	};
	const a = indexOf(selection.anchorNode);
	const b = indexOf(selection.focusNode);
	if (a === null || b === null) return null;
	return tokenRangeToTime(tokens, a, b);
}

function Token({
	span,
	overlay,
	note,
	active,
	selected,
	onSeek,
	onToggleCut,
	onApplyDiscretionary,
}: {
	span: TokenSpan;
	overlay: EffectiveOverlay;
	note: string | null;
	active: boolean;
	selected: boolean;
	onSeek: (seconds: number) => void;
	onToggleCut?: (cutIndex: number) => void;
	onApplyDiscretionary?: (index: number) => void;
}) {
	// Spacing tokens are not seek targets: rendering them as buttons would put
	// thousands of meaningless stops in the tab order of a long transcript.
	if (span.kind === "spacing") {
		let decorated: ReactNode = span.text;
		if (overlay?.kind === "cut") {
			decorated = <del className="text-fg-faint line-through">{span.text}</del>;
		}
		return (
			<span data-token-index={span.index} className="whitespace-pre-wrap">
				{decorated}
			</span>
		);
	}
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
	const handleClick = () => {
		// A struck word click only ever disables its cut (base cuts only —
		// re-enabling happens via undo, so a plain click can't silently re-cut).
		if (overlay?.kind === "cut" && onToggleCut) {
			const base = overlay.key.match(/^base-(\d+)$/);
			if (base?.[1] !== undefined) {
				onToggleCut(Number.parseInt(base[1], 10));
				return;
			}
		}
		if (overlay?.kind === "discretionary" && onApplyDiscretionary) {
			onApplyDiscretionary(overlay.entryIndex);
			return;
		}
		onSeek(span.start);
	};
	return (
		<button
			type="button"
			data-token-index={span.index}
			data-active={active || selected ? "" : undefined}
			className={`inline cursor-default whitespace-pre-wrap text-left ${
				active || selected ? "bg-surface-2" : ""
			}`}
			title={note ?? undefined}
			onClick={handleClick}
		>
			{inner}
		</button>
	);
}

/**
 * The transcript surface: paragraphs with mono gutter timecodes, effective
 * cuts gray-struck, unapplied discretionary amber-dotted (note on hover),
 * flags highlighted (seek-only). With a document + callbacks it edits: struck
 * word click toggles the cut off, amber click applies, drag-select + X adds a
 * manual cut. The active token carries the karaoke highlight.
 */
export function TranscriptPane({
	transcript,
	cuts,
	document: editDocument,
	activeTime,
	onSeek,
	onToggleCut,
	onApplyDiscretionary,
	onManualCut,
	selectedKey,
}: {
	transcript: Transcript;
	cuts: Cuts | null;
	document?: EditorDocument | null;
	activeTime?: number;
	onSeek: (seconds: number) => void;
	onToggleCut?: (cutIndex: number) => void;
	onApplyDiscretionary?: (index: number) => void;
	onManualCut?: (range: Range) => void;
	selectedKey?: string | null;
}) {
	const containerRef = useRef<HTMLDivElement>(null);
	const { spans, overlays, paragraphs } = useMemo(() => {
		const spans = buildTokenSpans(transcript.words);
		const doc = editDocument ?? EMPTY_DOCUMENT;
		const overlays: EffectiveOverlay[] = cuts
			? classifyEffective(spans, cuts, doc)
			: spans.map(() => null);
		return { spans, overlays, paragraphs: groupParagraphs(spans) };
	}, [transcript, cuts, editDocument]);

	useEffect(() => {
		if (!onManualCut) return;
		const handler = (event: KeyboardEvent) => {
			if (event.key !== "x" && event.key !== "X" && event.key !== "Delete") {
				return;
			}
			if (isEditableTarget(event.target)) return;
			const range = selectionTimeRange(spans);
			if (range === null) return;
			event.preventDefault();
			onManualCut(range);
			window.getSelection()?.removeAllRanges();
		};
		document.addEventListener("keydown", handler);
		return () => document.removeEventListener("keydown", handler);
	}, [onManualCut, spans]);

	// Scroll the selected region's first token into view when selection moves.
	useEffect(() => {
		if (selectedKey == null) return;
		containerRef.current
			?.querySelector("[data-active]")
			?.scrollIntoView?.({ block: "nearest" });
	}, [selectedKey]);

	let lastSpeaker: string | null = null;

	return (
		<div
			ref={containerRef}
			className="flex max-w-[65ch] flex-col gap-4 text-[15px] leading-[1.7]"
		>
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
								const active =
									activeTime !== undefined &&
									span.kind === "word" &&
									activeTime >= span.start &&
									activeTime < span.end;
								const selected =
									selectedKey != null &&
									overlay?.kind === "cut" &&
									overlay.key === selectedKey;
								return (
									<Token
										key={span.index}
										span={span}
										overlay={overlay}
										note={note}
										active={active}
										selected={selected}
										onSeek={onSeek}
										onToggleCut={onToggleCut}
										onApplyDiscretionary={onApplyDiscretionary}
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
