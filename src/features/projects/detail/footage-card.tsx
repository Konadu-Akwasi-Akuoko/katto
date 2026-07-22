import { FilmStripIcon } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PlanSteps } from "@/features/projects/detail/plan-steps";
import { importFiles } from "@/lib/ipc/ingest";
import { jobsKeys } from "@/lib/ipc/jobs";
import { listBundles, listFootage, pipelineKeys } from "@/lib/ipc/pipeline";
import { usePipelineStore } from "@/stores/pipeline";
import { useUiStore } from "@/stores/ui";

const VIDEO_EXTS = new Set(["mp4", "mov", "mts", "m4v"]);

/** Keep only paths with a video extension (folders and sidecars drop out). */
function videoPaths(paths: string[]): string[] {
	return paths.filter((p) =>
		VIDEO_EXTS.has(p.split(".").pop()?.toLowerCase() ?? ""),
	);
}

/**
 * The manual iPhone-footage path: drop video files anywhere over the detail
 * view and they run the same rename+verify ingest pipeline as a card import —
 * no watcher involvement. Uses the Tauri webview drag-drop event because a DOM
 * drop would not carry absolute paths.
 */
export function FootageCard({ slug }: { slug: string }) {
	const [over, setOver] = useState(false);
	const queryClient = useQueryClient();

	const drop = useMutation({
		mutationFn: (paths: string[]) => importFiles(slug, paths),
		onSuccess: (_job, paths) => {
			void queryClient.invalidateQueries({ queryKey: jobsKeys.all });
			toast.success(
				`Importing ${paths.length} ${paths.length === 1 ? "file" : "files"}`,
			);
		},
		onError: (err) => toast.error(err.message),
	});

	useEffect(() => {
		let cancelled = false;
		let webview: ReturnType<typeof getCurrentWebview>;
		try {
			webview = getCurrentWebview();
		} catch {
			// Outside a real Tauri webview (jsdom tests) there is no drop source.
			return;
		}
		const unlisten = webview.onDragDropEvent((event) => {
			if (cancelled) return;
			if (event.payload.type === "enter") setOver(true);
			else if (event.payload.type === "leave") setOver(false);
			else if (event.payload.type === "drop") {
				setOver(false);
				if (drop.isPending) return;
				const videos = videoPaths(event.payload.paths);
				if (videos.length === 0) {
					toast.error("No video files in that drop");
					return;
				}
				drop.mutate(videos);
			}
		});
		return () => {
			cancelled = true;
			void unlisten.then((un) => un());
		};
	}, [drop.isPending, drop.mutate]);

	return (
		<Card className={over ? "border-ember" : undefined}>
			<CardHeader>
				<CardTitle>Footage</CardTitle>
			</CardHeader>
			<CardContent className="flex flex-col gap-2">
				<FootageClipList slug={slug} />
				<div className="flex items-center gap-2 text-fg-muted">
					<FilmStripIcon size={20} />
					<span className="text-sm">
						Drop iPhone footage here to import it into this project
					</span>
				</div>
				<CutPlansList slug={slug} />
			</CardContent>
		</Card>
	);
}

/**
 * Existing `.kruproj` cut plans for this project: bundle name plus artifact
 * presence dots; clicking a row opens the read-only review surface.
 */
function CutPlansList({ slug }: { slug: string }) {
	const openCutEditor = useUiStore((s) => s.openCutEditor);
	const { data: bundles = [] } = useQuery({
		queryKey: pipelineKeys.bundles(slug),
		queryFn: () => listBundles(slug),
	});

	if (bundles.length === 0) return null;

	return (
		<div className="flex flex-col gap-1 border-t border-hairline pt-2">
			<span className="text-sm text-fg-muted">Cut plans</span>
			<ul className="flex flex-col">
				{bundles.map((bundle) => (
					<li key={bundle.path}>
						<button
							type="button"
							className="flex h-[30px] w-full cursor-default items-center gap-3 text-left"
							onClick={() => openCutEditor(bundle.path)}
						>
							<span className="flex-1 truncate font-mono text-xs tabular-nums">
								{bundle.name}
							</span>
							<span className="flex items-center gap-1 text-xs text-fg-muted">
								<span
									className={`size-1.5 rounded-full ${bundle.has_transcript ? "bg-done" : "bg-fg-faint"}`}
								/>
								transcript
							</span>
							<span className="flex items-center gap-1 text-xs text-fg-muted">
								<span
									className={`size-1.5 rounded-full ${bundle.has_cuts ? "bg-done" : "bg-fg-faint"}`}
								/>
								cuts
							</span>
						</button>
					</li>
				))}
			</ul>
		</div>
	);
}

/**
 * The project's footage clips, each with the rough-cut runner: a ghost "Plan
 * rough cut" action that swaps to the three-step indicator while its pipeline
 * job runs, then offers "Review cut plan".
 */
function FootageClipList({ slug }: { slug: string }) {
	const openCutEditor = useUiStore((s) => s.openCutEditor);
	const runs = usePipelineStore((s) => s.runs);
	const start = usePipelineStore((s) => s.start);
	const queryClient = useQueryClient();

	const { data: clips = [] } = useQuery({
		queryKey: pipelineKeys.footage(slug),
		queryFn: () => listFootage(slug),
	});

	if (clips.length === 0) return null;

	return (
		<ul className="flex flex-col">
			{clips.map((clip) => {
				const run = runs[clip.path];
				return (
					<li
						key={clip.path}
						className="flex flex-col border-b border-hairline py-1 last:border-b-0"
					>
						<div className="flex h-[30px] items-center gap-2">
							<span className="flex-1 truncate font-mono text-xs tabular-nums">
								{clip.name}
							</span>
							{!run && (
								<Button
									variant="ghost"
									size="sm"
									className="cursor-default"
									onClick={() => {
										void start(slug, clip.path).then(() => {
											void queryClient.invalidateQueries({
												queryKey: jobsKeys.all,
											});
										});
									}}
								>
									Plan rough cut
								</Button>
							)}
						</div>
						{run && <PlanSteps run={run} onReview={openCutEditor} />}
					</li>
				);
			})}
		</ul>
	);
}
