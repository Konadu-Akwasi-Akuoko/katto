import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { EditorDocument } from "@/features/editor/model/wire";
import { TranscriptPane } from "@/features/editor/transcript-pane";
import { fixtureCuts, fixtureTranscript } from "@/test/fixtures/editor";

const emptyDoc: EditorDocument = {
	toggledOff: [],
	appliedDiscretionary: [],
	manualCuts: [],
	boundaryAdjustments: [],
};

/** Put a real DOM selection across two rendered token texts. */
function selectTokens(fromText: string, toText: string) {
	const from = screen.getByText(fromText);
	const to = screen.getByText(toText);
	const range = document.createRange();
	range.setStartBefore(from);
	range.setEndAfter(to);
	const selection = window.getSelection();
	selection?.removeAllRanges();
	selection?.addRange(range);
}

describe("TranscriptPane", () => {
	it("clicking a word seeks to its start", () => {
		const onSeek = vi.fn();
		render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={null}
				onSeek={onSeek}
			/>,
		);
		fireEvent.click(screen.getByText("today"));
		expect(onSeek).toHaveBeenCalledWith(0.47);
	});

	it("cut tokens are marked struck-through for assistive tech", () => {
		render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={fixtureCuts}
				onSeek={() => {}}
			/>,
		);
		expect(screen.getByText("So").closest("del")).not.toBeNull();
	});

	it("discretionary tokens expose the note as a tooltip", () => {
		render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={fixtureCuts}
				onSeek={() => {}}
			/>,
		);
		expect(screen.getByText("today").closest("button")).toHaveAttribute(
			"title",
			"n",
		);
	});

	it("clicking a struck word toggles its cut off", () => {
		const onToggleCut = vi.fn();
		const onSeek = vi.fn();
		render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={fixtureCuts}
				document={emptyDoc}
				onSeek={onSeek}
				onToggleCut={onToggleCut}
			/>,
		);
		fireEvent.click(screen.getByText("So")); // inside cuts[0]
		expect(onToggleCut).toHaveBeenCalledWith(0);
		expect(onSeek).not.toHaveBeenCalled();
	});

	it("a toggled-off cut renders plain and clicks seek only", () => {
		const onToggleCut = vi.fn();
		const onSeek = vi.fn();
		render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={fixtureCuts}
				document={{ ...emptyDoc, toggledOff: [0] }}
				onSeek={onSeek}
				onToggleCut={onToggleCut}
			/>,
		);
		const word = screen.getByText("So");
		expect(word.closest("del")).toBeNull();
		fireEvent.click(word);
		expect(onToggleCut).not.toHaveBeenCalled();
		expect(onSeek).toHaveBeenCalledWith(0.12);
	});

	it("clicking an amber span applies the discretionary cut and it renders struck", () => {
		const onApply = vi.fn();
		const { rerender } = render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={fixtureCuts}
				document={emptyDoc}
				onSeek={() => {}}
				onApplyDiscretionary={onApply}
			/>,
		);
		fireEvent.click(screen.getByText("today")); // discretionary[0]
		expect(onApply).toHaveBeenCalledWith(0);
		rerender(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={fixtureCuts}
				document={{ ...emptyDoc, appliedDiscretionary: [0] }}
				onSeek={() => {}}
				onApplyDiscretionary={onApply}
			/>,
		);
		expect(screen.getByText("today").closest("del")).not.toBeNull();
	});

	it("selection + X creates a manual cut over the selected tokens", () => {
		const onManualCut = vi.fn();
		render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={fixtureCuts}
				document={emptyDoc}
				onSeek={() => {}}
				onManualCut={onManualCut}
			/>,
		);
		selectTokens("today", "Next");
		fireEvent.keyDown(document, { key: "x" });
		expect(onManualCut).toHaveBeenCalledWith({ start: 0.47, end: 3.2 });
	});

	it("karaoke highlight marks the active token", () => {
		render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={null}
				activeTime={0.5}
				onSeek={() => {}}
			/>,
		);
		expect(screen.getByText("today").closest("[data-active]")).not.toBeNull();
		expect(screen.getByText("So").closest("[data-active]")).toBeNull();
	});

	it("clicking a flagged word seeks and does not strike it", () => {
		const onSeek = vi.fn();
		render(
			<TranscriptPane
				transcript={fixtureTranscript}
				cuts={fixtureCuts}
				onSeek={onSeek}
			/>,
		);
		const flagged = screen.getByText("Next");
		expect(flagged.closest("del")).toBeNull();
		expect(flagged.closest("mark")).not.toBeNull();
		fireEvent.click(flagged);
		expect(onSeek).toHaveBeenCalledWith(2.9);
	});
});
