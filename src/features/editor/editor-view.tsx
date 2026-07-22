import { ArrowLeftIcon } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useRef } from "react";

import { Button } from "@/components/ui/button";
import { TranscriptPane } from "@/features/editor/transcript-pane";
import type { VideoPaneHandle } from "@/features/editor/video-pane";
import { VideoPane } from "@/features/editor/video-pane";
import type { Cut, Cuts } from "@/lib/ipc/pipeline";
import { openBundle, pipelineKeys } from "@/lib/ipc/pipeline";
import { IpcError } from "@/lib/ipc/result";
import { usePipelineStore } from "@/stores/pipeline";
import { useUiStore } from "@/stores/ui";

/** Plain-language copy for the typed open failures. */
function openErrorCopy(error: unknown): string {
	if (error instanceof IpcError && error.sourceMissing) {
		return `Source video not found at ${error.sourceMissing.expected_path}. Relocation arrives with the editor phase.`;
	}
	return error instanceof Error ? error.message : String(error);
}

/**
 * Read-only cut review: video pane pinned left, transcript scrolling right.
 * Reached from a finished pipeline run or the footage card's cut-plan list;
 * lives inside the projects surface (back returns to project detail).
 */
export function EditorView({ bundlePath }: { bundlePath: string }) {
	const closeCutEditor = useUiStore((s) => s.closeCutEditor);
	const videoRef = useRef<VideoPaneHandle>(null);

	const bundle = useQuery({
		queryKey: pipelineKeys.bundle(bundlePath),
		queryFn: () => openBundle(bundlePath),
		retry: false,
	});

	// Opened mid-run (transcript ready, planner still streaming): cuts.json is
	// absent, so hydrate incremental cuts live from this bundle's pipeline run.
	// The `done` event invalidates the bundle query and cuts.json takes over.
	const liveCutsSoFar = usePipelineStore(
		(s) =>
			Object.values(s.runs).find((r) => r.bundlePath === bundlePath)
				?.cutsSoFar ?? null,
	);
	const duration = bundle.data?.duration_secs;
	const liveCuts = useMemo<Cuts | null>(() => {
		if (!liveCutsSoFar || liveCutsSoFar.length === 0) return null;
		return {
			source_duration_secs: duration ?? 0,
			cuts: liveCutsSoFar,
			discretionary: [],
			flags: [],
			total_cut_secs: liveCutsSoFar.reduce(
				(sum: number, c: Cut) => sum + (c.end - c.start),
				0,
			),
		};
	}, [liveCutsSoFar, duration]);

	const name =
		bundlePath
			.split("/")
			.pop()
			?.replace(/\.kruproj$/, "") ?? "";

	return (
		<div className="flex h-full min-h-0 flex-col gap-3 p-4">
			<div className="flex items-center gap-2">
				<Button
					variant="ghost"
					size="icon-sm"
					className="cursor-default"
					aria-label="Back to project"
					onClick={closeCutEditor}
				>
					<ArrowLeftIcon size={16} />
				</Button>
				<h1 className="font-serif text-lg">{name}</h1>
				<span className="font-mono text-xs tabular-nums text-fg-muted">
					{bundlePath}
				</span>
			</div>

			{bundle.isPending && (
				<p className="text-sm text-fg-muted">Opening bundle…</p>
			)}
			{bundle.isError && (
				<p className="text-sm text-failed">{openErrorCopy(bundle.error)}</p>
			)}
			{bundle.isSuccess && (
				<div className="grid min-h-0 flex-1 grid-cols-[minmax(320px,2fr)_3fr] gap-4">
					<div className="min-h-0">
						<VideoPane ref={videoRef} sourcePath={bundle.data.source_path} />
					</div>
					<div className="min-h-0 overflow-y-auto pr-2">
						<TranscriptPane
							transcript={bundle.data.transcript}
							cuts={bundle.data.cuts ?? liveCuts}
							onSeek={(seconds) => videoRef.current?.seek(seconds)}
						/>
					</div>
				</div>
			)}
		</div>
	);
}
