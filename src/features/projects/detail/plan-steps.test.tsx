import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PlanSteps } from "@/features/projects/detail/plan-steps";
import { initialRun, reduceEvent } from "@/stores/pipeline";

function midRun() {
	let run = initialRun("slug", "/p/footage/clip.mp4");
	run = reduceEvent(run, {
		type: "stage",
		name: "transcribing",
		progress: 0.4,
	});
	return run;
}

describe("PlanSteps", () => {
	it("renders all three step labels", () => {
		render(<PlanSteps run={midRun()} />);
		expect(screen.getByText("Extracting audio")).toBeInTheDocument();
		expect(screen.getByText("Transcribing")).toBeInTheDocument();
		expect(screen.getByText("Detecting cuts")).toBeInTheDocument();
	});

	it("shows an elapsed timer on the active step and a dash on pending", () => {
		render(<PlanSteps run={midRun()} />);
		// active step's detail is m:ss; pending is an em dash
		expect(screen.getByText(/^\d+:\d{2}$/)).toBeInTheDocument();
		expect(screen.getByText("—")).toBeInTheDocument();
	});

	it("shows a live cut count while detecting cuts", () => {
		let run = midRun();
		run = reduceEvent(run, {
			type: "stage",
			name: "detecting_cuts",
			progress: 0.66,
		});
		run = reduceEvent(run, {
			type: "cuts_partial",
			cuts_so_far: [
				{ start: 1, end: 2, reason: "filler", excerpt: "um" },
				{ start: 3, end: 4, reason: "stutter", excerpt: "I-I" },
			],
		});
		render(<PlanSteps run={run} />);
		expect(screen.getByText("2 cuts")).toBeInTheDocument();
	});

	it("failed run shows plain-language error copy", () => {
		let run = midRun();
		run = reduceEvent(run, {
			type: "failed",
			error: "elevenlabs auth: bad key",
		});
		render(<PlanSteps run={run} />);
		expect(screen.getByText(/elevenlabs auth: bad key/)).toBeInTheDocument();
	});

	it("subprocess failure offers API-key mode copy", () => {
		let run = midRun();
		run = reduceEvent(run, {
			type: "failed",
			error: "claude subprocess: claude exited with signal 9",
		});
		render(<PlanSteps run={run} />);
		expect(
			screen.getByText(/open Settings to add an Anthropic API key/),
		).toBeInTheDocument();
	});

	it("finished run offers the review button with the bundle path", () => {
		const onReview = vi.fn();
		let run = midRun();
		run = reduceEvent(run, {
			type: "done",
			bundle_path: "/p/audio/clip.kruproj",
		});
		render(<PlanSteps run={run} onReview={onReview} />);
		fireEvent.click(screen.getByRole("button", { name: "Review cut plan" }));
		expect(onReview).toHaveBeenCalledWith("/p/audio/clip.kruproj");
	});
});
