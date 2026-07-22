import { Channel } from "@tauri-apps/api/core";
import type {
	BundleData,
	BundleSummary,
	Cut,
	Cuts,
	FailureKind,
	FootageClip,
	Job,
	PipelineEvent,
	StageName,
	Transcript,
	WordEntry,
} from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type {
	BundleData,
	BundleSummary,
	Cut,
	Cuts,
	FailureKind,
	FootageClip,
	PipelineEvent,
	StageName,
	Transcript,
	WordEntry,
};

export const pipelineKeys = {
	all: ["pipeline"] as const,
	footage: (slug: string) => [...pipelineKeys.all, "footage", slug] as const,
	bundles: (slug: string) => [...pipelineKeys.all, "bundles", slug] as const,
	bundle: (path: string) => [...pipelineKeys.all, "bundle", path] as const,
};

/**
 * Start the rough-cut pipeline job for one footage clip. Progress streams over
 * the channel; the returned Job row is already queued.
 */
export const planRoughCut = (
	projectSlug: string,
	footagePath: string,
	onEvent: (e: PipelineEvent) => void,
): Promise<Job> => {
	const channel = new Channel<PipelineEvent>();
	channel.onmessage = onEvent;
	return unwrap(commands.planRoughCut(projectSlug, footagePath, channel));
};

/** Load a full bundle (manifest + transcript + cuts + edits) in one call. */
export const openBundle = (path: string): Promise<BundleData> =>
	unwrap(commands.openBundle(path));

/** List a project's `.kruproj` bundles with artifact presence. */
export const listBundles = (projectSlug: string): Promise<BundleSummary[]> =>
	unwrap(commands.listBundles(projectSlug));

/** List a project's footage clips (video files under footage/). */
export const listFootage = (projectSlug: string): Promise<FootageClip[]> =>
	unwrap(commands.listFootage(projectSlug));
