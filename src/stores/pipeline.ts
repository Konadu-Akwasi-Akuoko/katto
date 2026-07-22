import { create } from "zustand";
import type { Cut, PipelineEvent, StageName } from "@/lib/ipc/bindings.gen";
import { planRoughCut } from "@/lib/ipc/pipeline";

export type StepState = "pending" | "active" | "done" | "failed";

export type PipelineRun = {
	jobId: string | null;
	projectSlug: string;
	footagePath: string;
	bundlePath: string | null;
	steps: Record<StageName, StepState>;
	/** 0..1 within the whole run, from the latest stage event. */
	stageProgress: number;
	cutsSoFar: Cut[];
	error: string | null;
	finished: boolean;
	/** Epoch ms when the run started (drives the elapsed readout). */
	startedAt: number;
};

const STAGE_ORDER: StageName[] = [
	"extracting_audio",
	"transcribing",
	"detecting_cuts",
];

/** A fresh run: first step active, everything else pending. */
export function initialRun(
	projectSlug: string,
	footagePath: string,
): PipelineRun {
	return {
		jobId: null,
		projectSlug,
		footagePath,
		bundlePath: null,
		steps: {
			extracting_audio: "active",
			transcribing: "pending",
			detecting_cuts: "pending",
		},
		stageProgress: 0,
		cutsSoFar: [],
		error: null,
		finished: false,
		startedAt: Date.now(),
	};
}

/** Pure event reduction — exported for tests. */
export function reduceEvent(run: PipelineRun, e: PipelineEvent): PipelineRun {
	switch (e.type) {
		case "stage": {
			const activeIndex = STAGE_ORDER.indexOf(e.name);
			const steps = { ...run.steps };
			STAGE_ORDER.forEach((stage, i) => {
				if (i < activeIndex) steps[stage] = "done";
				else if (i === activeIndex) steps[stage] = "active";
			});
			return { ...run, steps, stageProgress: e.progress };
		}
		case "transcript_ready":
			return { ...run, bundlePath: e.bundle_path };
		case "cuts_partial":
			return { ...run, cutsSoFar: e.cuts_so_far };
		case "done": {
			const steps = { ...run.steps };
			for (const stage of STAGE_ORDER) steps[stage] = "done";
			return {
				...run,
				steps,
				bundlePath: e.bundle_path,
				finished: true,
				stageProgress: 1,
			};
		}
		case "failed": {
			const steps = { ...run.steps };
			for (const stage of STAGE_ORDER) {
				if (steps[stage] === "active") steps[stage] = "failed";
			}
			return { ...run, steps, error: e.error, finished: true };
		}
	}
}

type PipelineState = {
	/** Live runs keyed by footage path (one run per clip at a time). */
	runs: Record<string, PipelineRun>;
	start: (projectSlug: string, footagePath: string) => Promise<void>;
	reset: (footagePath: string) => void;
};

/**
 * Pipeline run state lives here (not TanStack Query) because it is streamed
 * job state, reachable from project detail and future surfaces without
 * feature-to-feature imports. Read via selectors only.
 */
export const usePipelineStore = create<PipelineState>((set, get) => ({
	runs: {},
	start: async (projectSlug, footagePath) => {
		const existing = get().runs[footagePath];
		if (existing && !existing.finished) return; // one run per clip
		set((s) => ({
			runs: { ...s.runs, [footagePath]: initialRun(projectSlug, footagePath) },
		}));
		const apply = (e: PipelineEvent) =>
			set((s) => {
				const run = s.runs[footagePath];
				if (!run) return s;
				return { runs: { ...s.runs, [footagePath]: reduceEvent(run, e) } };
			});
		try {
			const job = await planRoughCut(projectSlug, footagePath, apply);
			set((s) => {
				const run = s.runs[footagePath];
				if (!run) return s;
				return {
					runs: { ...s.runs, [footagePath]: { ...run, jobId: job.id } },
				};
			});
		} catch (err) {
			apply({
				type: "failed",
				error: err instanceof Error ? err.message : String(err),
			});
		}
	},
	reset: (footagePath) =>
		set((s) => {
			const { [footagePath]: _dropped, ...rest } = s.runs;
			return { runs: rest };
		}),
}));
