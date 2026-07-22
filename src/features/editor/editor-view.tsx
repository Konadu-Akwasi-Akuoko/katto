import { ArrowLeftIcon, PauseIcon, PlayIcon } from "@phosphor-icons/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "zustand";

import { Button } from "@/components/ui/button";
import { ExportDialog } from "@/features/editor/export-dialog";
import {
	coalesceRanges,
	effectiveCutRanges,
	keptDuration,
} from "@/features/editor/model/kept-ranges";
import type { Viewport } from "@/features/editor/model/timeline-geometry";
import { clampViewport } from "@/features/editor/model/timeline-geometry";
import { buildTokenSpans } from "@/features/editor/model/tokens";
import type { Range } from "@/features/editor/model/wire";
import { fromWireEdits } from "@/features/editor/model/wire";
import { RelocateDialog } from "@/features/editor/relocate-dialog";
import { createAutosave } from "@/features/editor/store/autosave";
import type { EditorStore } from "@/features/editor/store/editor-store";
import {
	createEditorStore,
	documentOf,
	keyToDragTarget,
} from "@/features/editor/store/editor-store";
import { TimelinePane } from "@/features/editor/timeline-pane";
import { TranscriptPane } from "@/features/editor/transcript-pane";
import { isEditableTarget, keyToAction } from "@/features/editor/transport";
import { useTransport } from "@/features/editor/use-transport";
import type { VideoPaneHandle } from "@/features/editor/video-pane";
import { VideoPane } from "@/features/editor/video-pane";
import { Waveform } from "@/features/editor/waveform";
import { generateThumbs, saveEdits } from "@/lib/ipc/editor";
import type { BundleData, Cut, Cuts } from "@/lib/ipc/pipeline";
import { openBundle, pipelineKeys } from "@/lib/ipc/pipeline";
import { IpcError } from "@/lib/ipc/result";
import { usePipelineStore } from "@/stores/pipeline";
import { useUiStore } from "@/stores/ui";

/** Plain-language copy for the typed open failures. */
function openErrorCopy(error: unknown): string {
	if (error instanceof IpcError && error.sourceMissing) {
		return `Source video not found at ${error.sourceMissing.expected_path}.`;
	}
	return error instanceof Error ? error.message : String(error);
}

/** m:ss.s mono readout. */
function readout(seconds: number): string {
	const clamped = Math.max(0, seconds);
	const m = Math.floor(clamped / 60);
	const s = clamped - m * 60;
	return `${m}:${s.toFixed(1).padStart(4, "0")}`;
}

/** Source time -> kept-only time (subtract removed span time before t). */
function keptTimeOf(t: number, coalesced: Range[]): number {
	let removed = 0;
	for (const cut of coalesced) {
		if (cut.end <= t) removed += cut.end - cut.start;
		else if (cut.start < t) removed += t - cut.start;
	}
	return t - removed;
}

/**
 * The editor surface. Mid-run (no cuts.json yet) it stays the read-only
 * review: transcript + live incremental cuts. Once cuts.json exists it
 * becomes the full three-pane editor with document store, undo, auto-save,
 * timeline, and transport.
 */
export function EditorView({ bundlePath }: { bundlePath: string }) {
	const closeCutEditor = useUiStore((s) => s.closeCutEditor);
	const queryClient = useQueryClient();
	const [relocating, setRelocating] = useState(false);

	const bundle = useQuery({
		queryKey: pipelineKeys.bundle(bundlePath),
		queryFn: () => openBundle(bundlePath),
		retry: false,
	});

	// Opened mid-run (transcript ready, planner still streaming): cuts.json is
	// absent, so hydrate incremental cuts live from this bundle's pipeline run.
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
				<div className="flex flex-col items-start gap-2">
					<p className="text-sm text-failed">{openErrorCopy(bundle.error)}</p>
					{bundle.error instanceof IpcError && bundle.error.sourceMissing && (
						<Button
							variant="secondary"
							size="sm"
							className="cursor-default"
							onClick={() => setRelocating(true)}
						>
							Locate the source…
						</Button>
					)}
				</div>
			)}
			{relocating &&
				bundle.error instanceof IpcError &&
				bundle.error.sourceMissing && (
					<RelocateDialog
						bundlePath={bundlePath}
						info={bundle.error.sourceMissing}
						onClose={() => setRelocating(false)}
						onRelocated={() => {
							setRelocating(false);
							void queryClient.invalidateQueries({
								queryKey: pipelineKeys.bundle(bundlePath),
							});
						}}
					/>
				)}
			{bundle.isSuccess &&
				(bundle.data.cuts ? (
					<EditingSurface bundlePath={bundlePath} data={bundle.data} />
				) : (
					<ReadOnlySurface data={bundle.data} liveCuts={liveCuts} />
				))}
		</div>
	);
}

/** Mid-run review: video left, transcript right, no editing. */
function ReadOnlySurface({
	data,
	liveCuts,
}: {
	data: BundleData;
	liveCuts: Cuts | null;
}) {
	const videoRef = useRef<VideoPaneHandle>(null);
	return (
		<div className="grid min-h-0 flex-1 grid-cols-[minmax(320px,2fr)_3fr] gap-4">
			<div className="min-h-0">
				<VideoPane ref={videoRef} sourcePath={data.source_path} />
			</div>
			<div className="min-h-0 overflow-y-auto pr-2">
				<TranscriptPane
					transcript={data.transcript}
					cuts={liveCuts}
					onSeek={(seconds) => videoRef.current?.seek(seconds)}
				/>
			</div>
		</div>
	);
}

function EditingSurface({
	bundlePath,
	data,
}: {
	bundlePath: string;
	data: BundleData;
}) {
	const cuts = data.cuts;
	if (cuts === null) throw new Error("EditingSurface requires cuts");
	return <Editing bundlePath={bundlePath} data={data} cuts={cuts} />;
}

function Editing({
	bundlePath,
	data,
	cuts,
}: {
	bundlePath: string;
	data: BundleData;
	cuts: Cuts;
}) {
	const videoRef = useRef<VideoPaneHandle>(null);
	const queryClient = useQueryClient();
	const fps = data.frame_rate;
	const duration = data.duration_secs;

	const tokens = useMemo(
		() => buildTokenSpans(data.transcript.words),
		[data.transcript],
	);
	const store: EditorStore = useMemo(
		() =>
			createEditorStore({
				...fromWireEdits(data.edits, fps),
				cuts,
				tokens,
			}),
		[data.edits, fps, cuts, tokens],
	);
	const state = useStore(store);
	const doc = documentOf(state);

	const ranges = useMemo(() => effectiveCutRanges(cuts, doc), [cuts, doc]);
	const coalesced = useMemo(() => coalesceRanges(ranges), [ranges]);
	const discretionaryUnapplied = useMemo(
		() =>
			cuts.discretionary
				.map((d, i) => ({ start: d.start, end: d.end, index: i }))
				.filter((d) => !doc.appliedDiscretionary.includes(d.index)),
		[cuts, doc.appliedDiscretionary],
	);

	// Auto-save: THE debounced bridge call; failures pause with a banner.
	const [saveBanner, setSaveBanner] = useState<string | null>(null);
	const autosaveRef = useRef<ReturnType<typeof createAutosave> | null>(null);
	useEffect(() => {
		const autosave = createAutosave({
			store,
			fps,
			save: (edits) => saveEdits(bundlePath, edits),
			onPaused: (message) =>
				setSaveBanner(
					`Auto-save paused: ${message}. Edits stay in memory; the next edit retries.`,
				),
		});
		autosaveRef.current = autosave;
		return () => {
			autosaveRef.current = null;
			void autosave.flushNow().catch(() => {});
			autosave.dispose();
		};
	}, [store, fps, bundlePath]);

	const rangesRef = useRef(ranges);
	rangesRef.current = ranges;
	const transport = useTransport({
		videoRef,
		getCutRanges: () => rangesRef.current,
		fps,
	});
	const { showOriginal, currentTime, playing, dispatch } = transport;

	// Global editor keys; the transcript pane owns X/Delete (needs selection).
	useEffect(() => {
		const handler = (event: KeyboardEvent) => {
			if (isEditableTarget(event.target)) return;
			const action = keyToAction(event);
			if (action === null || action.kind === "manual-cut") return;
			event.preventDefault();
			if (action.kind === "undo") {
				store.temporal.getState().undo();
				return;
			}
			if (action.kind === "redo") {
				store.temporal.getState().redo();
				return;
			}
			dispatch(action);
		};
		window.addEventListener("keydown", handler);
		return () => window.removeEventListener("keydown", handler);
	}, [store, dispatch]);

	// Timeline viewport (canvas owns it; waveform mirrors it one-way).
	const [viewport, setViewport] = useState<Viewport>({
		pxPerSec: 20,
		scrollSec: 0,
		widthPx: 800,
	});
	const onViewportChange = useCallback(
		(vp: Viewport) => setViewport(clampViewport(vp, duration)),
		[duration],
	);
	const [selectedKey, setSelectedKey] = useState<string | null>(null);

	const [exportOpen, setExportOpen] = useState(false);
	const [relocateInfo, setRelocateInfo] = useState<{
		expected_path: string;
		filename: string;
		duration_secs: number;
	} | null>(null);

	// D21 close guard: while a save is pending/saving, hold the close, flush,
	// then destroy (the Rust side keeps katto in the tray either way).
	useEffect(() => {
		const win = getCurrentWindow();
		const unlisten = win.onCloseRequested(async (event) => {
			const autosave = autosaveRef.current;
			if (!autosave) return;
			const s = autosave.state();
			if (s === "pending" || s === "saving") {
				event.preventDefault();
				await autosave.flushNow().catch(() => {});
				await win.destroy();
			}
		});
		return () => {
			void unlisten.then((u) => u());
		};
	}, []);

	// Thumbnails: regenerate once when the strip probe misses.
	const [thumbsVersion, setThumbsVersion] = useState(0);
	const thumbsRequested = useRef(false);
	const onThumbsMissing = useCallback(() => {
		if (thumbsRequested.current) return;
		thumbsRequested.current = true;
		void generateThumbs(bundlePath, (p) => {
			if (p.progress >= 1) setThumbsVersion((v) => v + 1);
		}).catch(() => {
			// The job row + events carry the failure; the strip stays empty.
		});
		void queryClient.invalidateQueries({ queryKey: ["jobs"] });
	}, [bundlePath, queryClient]);

	const seek = useCallback((t: number) => videoRef.current?.seek(t), []);

	const total = showOriginal ? duration : keptDuration(ranges, duration);
	const displayTime = showOriginal
		? currentTime
		: keptTimeOf(currentTime, coalesced);

	return (
		<div className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)_auto]">
			<div className="grid min-h-0 grid-cols-[minmax(420px,1fr)_40%] gap-4">
				<div className="min-h-0 overflow-y-auto pr-2">
					{saveBanner !== null && (
						<p
							className="mb-2 border border-warn/40 bg-warn/10 px-3 py-2 text-sm"
							style={{ backgroundImage: "none" }}
						>
							{saveBanner}
						</p>
					)}
					<TranscriptPane
						transcript={data.transcript}
						cuts={cuts}
						document={doc}
						activeTime={currentTime}
						onSeek={seek}
						onToggleCut={(i) => state.toggleCut(i)}
						onApplyDiscretionary={(i) => state.applyDiscretionary(i)}
						onManualCut={(range) => state.addManualCut(range)}
						selectedKey={selectedKey}
					/>
				</div>
				<div className="min-h-0">
					<VideoPane
						ref={videoRef}
						sourcePath={data.source_path}
						onTimeUpdate={transport.onTimeUpdate}
						onPlayingChange={transport.onPlayingChange}
					/>
				</div>
			</div>

			<div className="flex flex-col border-t border-hairline pt-1">
				<TimelinePane
					duration={duration}
					fps={fps}
					tokens={tokens}
					ranges={ranges}
					discretionaryUnapplied={discretionaryUnapplied}
					selectedKey={selectedKey}
					currentTime={currentTime}
					viewport={viewport}
					bundlePath={bundlePath}
					thumbsVersion={thumbsVersion}
					onThumbsMissing={onThumbsMissing}
					onViewportChange={onViewportChange}
					onSeek={seek}
					onSelect={setSelectedKey}
					onDragBegin={() => state.beginDrag()}
					onDrag={(target, edge, t) => state.dragBoundary(target, edge, t)}
					onDragEnd={(commit) =>
						commit ? state.commitDrag() : state.cancelDrag()
					}
					onMarqueeCut={(range) => state.addManualCut(range)}
				/>

				<Waveform
					audioUrl={convertFileSrc(`${bundlePath}/cached_audio.wav`)}
					ranges={ranges}
					currentTime={currentTime}
					viewport={viewport}
					selectedKey={selectedKey}
					onSeek={seek}
					onDragBegin={() => state.beginDrag()}
					onDrag={(key, edge, t) => {
						const target = keyToDragTarget(key);
						if (target) state.dragBoundary(target, edge, t);
					}}
					onDragEnd={() => state.commitDrag()}
					onSelect={setSelectedKey}
				/>

				<div className="flex h-8 items-center gap-2">
					<Button
						variant="ghost"
						size="icon-sm"
						className="cursor-default"
						aria-label={playing ? "Pause" : "Play"}
						onClick={() => dispatch({ kind: "toggle-play" })}
					>
						{playing ? <PauseIcon size={16} /> : <PlayIcon size={16} />}
					</Button>
					<span className="font-mono text-xs tabular-nums text-fg-muted">
						{readout(displayTime)} / {readout(total)}
					</span>
					<Button
						variant="ghost"
						size="sm"
						className="cursor-default"
						aria-pressed={showOriginal}
						onClick={() => dispatch({ kind: "toggle-original" })}
					>
						Show original
					</Button>
					<div className="ml-auto flex items-center gap-2">
						<Button
							variant="secondary"
							size="sm"
							className="cursor-default"
							onClick={() => setExportOpen(true)}
						>
							Export…
						</Button>
						<input
							type="range"
							aria-label="Timeline zoom"
							min={2}
							max={200}
							value={viewport.pxPerSec}
							onChange={(e) =>
								onViewportChange({
									...viewport,
									pxPerSec: Number(e.target.value),
								})
							}
						/>
					</div>
				</div>
			</div>

			{exportOpen && (
				<ExportDialog
					bundlePath={bundlePath}
					flush={() => autosaveRef.current?.flushNow() ?? Promise.resolve()}
					onClose={() => setExportOpen(false)}
					onExported={() => {
						void queryClient.invalidateQueries({ queryKey: ["events"] });
					}}
					onSourceMissing={(info) => {
						setExportOpen(false);
						setRelocateInfo(info);
					}}
				/>
			)}
			{relocateInfo !== null && (
				<RelocateDialog
					bundlePath={bundlePath}
					info={relocateInfo}
					onClose={() => setRelocateInfo(null)}
					onRelocated={() => {
						setRelocateInfo(null);
						void queryClient.invalidateQueries({
							queryKey: pipelineKeys.bundle(bundlePath),
						});
					}}
				/>
			)}
		</div>
	);
}
