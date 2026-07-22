import { describe, expect, it } from "vitest";
import { initialRun, reduceEvent } from "./pipeline";

describe("pipeline event reduction", () => {
	it("marks earlier stages done when a later stage starts", () => {
		let run = initialRun("slug", "/f/clip.mp4");
		run = reduceEvent(run, {
			type: "stage",
			name: "transcribing",
			progress: 0,
		});
		expect(run.steps.extracting_audio).toBe("done");
		expect(run.steps.transcribing).toBe("active");
	});

	it("stores bundle path on transcript_ready", () => {
		let run = initialRun("slug", "/f/clip.mp4");
		run = reduceEvent(run, {
			type: "transcript_ready",
			bundle_path: "/p/audio/clip.kruproj",
		});
		expect(run.bundlePath).toBe("/p/audio/clip.kruproj");
	});

	it("accumulates cuts_partial and finishes on done", () => {
		let run = initialRun("slug", "/f/clip.mp4");
		run = reduceEvent(run, {
			type: "cuts_partial",
			cuts_so_far: [{ start: 1, end: 2, reason: "filler", excerpt: "um" }],
		});
		expect(run.cutsSoFar).toHaveLength(1);
		run = reduceEvent(run, { type: "done", bundle_path: "/b" });
		expect(run.finished).toBe(true);
		expect(run.steps.detecting_cuts).toBe("done");
	});

	it("failed marks the active step failed and records the error", () => {
		let run = initialRun("slug", "/f/clip.mp4");
		run = reduceEvent(run, {
			type: "stage",
			name: "transcribing",
			progress: 0.1,
		});
		run = reduceEvent(run, {
			type: "failed",
			error: "elevenlabs auth: bad key",
		});
		expect(run.steps.transcribing).toBe("failed");
		expect(run.error).toContain("elevenlabs");
	});

	it("stage events carry overall progress", () => {
		let run = initialRun("slug", "/f/clip.mp4");
		run = reduceEvent(run, {
			type: "stage",
			name: "detecting_cuts",
			progress: 0.66,
		});
		expect(run.steps.extracting_audio).toBe("done");
		expect(run.steps.transcribing).toBe("done");
		expect(run.steps.detecting_cuts).toBe("active");
		expect(run.stageProgress).toBe(0.66);
	});
});
