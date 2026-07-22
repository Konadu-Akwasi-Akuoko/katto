import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TranscriptPane } from "@/features/editor/transcript-pane";
import { fixtureCuts, fixtureTranscript } from "@/test/fixtures/editor";

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
