import { describe, expect, it } from "vitest";
import { pipelineKeys } from "@/lib/ipc/pipeline";
import { initialRun, invalidationsFor, reduceEvent } from "@/stores/pipeline";

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

	it("failed marks the active step failed and records error plus kind", () => {
		let run = initialRun("slug", "/f/clip.mp4");
		run = reduceEvent(run, {
			type: "stage",
			name: "transcribing",
			progress: 0.1,
		});
		run = reduceEvent(run, {
			type: "failed",
			error: "elevenlabs auth: bad key",
			kind: "auth",
		});
		expect(run.steps.transcribing).toBe("failed");
		expect(run.error).toContain("elevenlabs");
		expect(run.errorKind).toBe("auth");
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

describe("query invalidations per event", () => {
	it("transcript_ready refreshes the bundles list", () => {
		expect(
			invalidationsFor({ type: "transcript_ready", bundle_path: "/b" }, "slug"),
		).toContainEqual(pipelineKeys.bundles("slug"));
	});

	it("done refreshes bundles list and the bundle itself", () => {
		const keys = invalidationsFor({ type: "done", bundle_path: "/b" }, "slug");
		expect(keys).toContainEqual(pipelineKeys.bundles("slug"));
		expect(keys).toContainEqual(pipelineKeys.bundle("/b"));
	});

	it("mid-stream events invalidate nothing", () => {
		expect(
			invalidationsFor(
				{ type: "stage", name: "transcribing", progress: 0.3 },
				"slug",
			),
		).toHaveLength(0);
		expect(
			invalidationsFor({ type: "cuts_partial", cuts_so_far: [] }, "slug"),
		).toHaveLength(0);
	});
});
