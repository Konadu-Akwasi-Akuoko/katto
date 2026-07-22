import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import type { ExportPreview, ExportResult, NleTarget } from "@/lib/ipc/editor";
import {
	exportTimeline,
	openInFcp,
	previewExport,
	renderMp4,
	revealTimeline,
} from "@/lib/ipc/editor";
import { openInResolve, resolveAvailable } from "@/lib/ipc/resolve";
import { IpcError } from "@/lib/ipc/result";

const NLE_LABELS: Array<{ value: NleTarget; label: string }> = [
	{ value: "final_cut", label: "Final Cut Pro" },
	{ value: "resolve", label: "DaVinci Resolve" },
	{ value: "premiere", label: "Premiere Pro" },
];

/**
 * Export flow: preview names the exact next files, the NLE pick is forced on
 * first export (then sticky), FCPXML + captions always write together, the MP4
 * render is an optional job. Success swaps to a done state whose primary
 * action opens the selected NLE (Final Cut) or reveals in Finder.
 */
export function ExportDialog({
	bundlePath,
	flush,
	onClose,
	onExported,
	onSourceMissing,
}: {
	bundlePath: string;
	/** Autosave flushNow — edits must be on disk before the engine reads them. */
	flush?: () => Promise<void>;
	onClose(): void;
	onExported(result: ExportResult): void;
	onSourceMissing?(info: {
		expected_path: string;
		filename: string;
		duration_secs: number;
	}): void;
}) {
	const [preview, setPreview] = useState<ExportPreview | null>(null);
	const [target, setTarget] = useState<NleTarget | null>(null);
	const [alsoRender, setAlsoRender] = useState(false);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [flushWarning, setFlushWarning] = useState(false);
	// Failures AFTER the export succeeded (render spawn, open/reveal) — shown
	// in whichever view is up; never silently discarded.
	const [postError, setPostError] = useState<string | null>(null);
	const [done, setDone] = useState<ExportResult | null>(null);
	// One-shot probe, not app state: Resolve being installed can't change
	// while the dialog is up, and a failed probe just hides the button.
	const [resolveInstalled, setResolveInstalled] = useState(false);
	useEffect(() => {
		let cancelled = false;
		resolveAvailable()
			.then((available) => {
				if (!cancelled) setResolveInstalled(available);
			})
			.catch(() => {});
		return () => {
			cancelled = true;
		};
	}, []);

	useEffect(() => {
		let cancelled = false;
		void (async () => {
			try {
				await flush?.();
			} catch {
				// The export command reads whatever landed on disk — say so
				// instead of silently exporting stale edits.
				if (!cancelled) setFlushWarning(true);
			}
			try {
				const p = await previewExport(bundlePath);
				if (cancelled) return;
				setPreview(p);
				setTarget(p.default_nle);
			} catch (e) {
				if (!cancelled) setError(e instanceof Error ? e.message : String(e));
			}
		})();
		return () => {
			cancelled = true;
		};
	}, [bundlePath, flush]);

	const runExport = async () => {
		if (target === null) return;
		setBusy(true);
		setError(null);
		try {
			const result = await exportTimeline(bundlePath, target, false);
			setDone(result);
			onExported(result);
			if (alsoRender) {
				// Progress lives in the jobs surface, not the dialog — but a
				// refused spawn (busy guard, bad bundle) surfaces here.
				void renderMp4(bundlePath, () => {}).catch((e) =>
					setPostError(
						`Render did not start: ${e instanceof Error ? e.message : String(e)}`,
					),
				);
			}
		} catch (e) {
			if (e instanceof IpcError && e.sourceMissing && onSourceMissing) {
				onSourceMissing(e.sourceMissing);
				onClose();
				return;
			}
			setError(e instanceof Error ? e.message : String(e));
		} finally {
			setBusy(false);
		}
	};

	const nextName =
		preview === null ? null : `${preview.slug}-v${preview.version}.fcpxml`;

	return (
		<Dialog open onOpenChange={(open) => !open && onClose()}>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>Export timeline</DialogTitle>
					<DialogDescription>
						Versioned into the project's timelines folder — earlier versions are
						never touched.
					</DialogDescription>
				</DialogHeader>

				{flushWarning && (
					<p className="text-sm text-warn">
						The latest edits could not be saved — this export uses the last
						saved state.
					</p>
				)}
				{done === null ? (
					<div className="flex flex-col gap-4">
						{preview !== null && (
							<div className="flex flex-col gap-1">
								<span className="font-mono text-sm tabular-nums">
									{nextName}
								</span>
								{preview.version > 1 && (
									<span className="text-xs text-fg-muted">
										v1–v{preview.version - 1} stay untouched.
									</span>
								)}
							</div>
						)}

						<div className="flex flex-col gap-2">
							<div className="flex items-center gap-2 text-sm">
								<Checkbox
									checked
									disabled
									aria-label="Timeline (FCPXML 1.11)"
								/>
								<span>Timeline (FCPXML 1.11)</span>
							</div>
							<div className="flex items-center gap-2 text-sm">
								<Checkbox checked disabled aria-label="Captions (SRT + VTT)" />
								<span>Captions (SRT + VTT)</span>
							</div>
							<div className="flex items-center gap-2 text-sm">
								<Checkbox
									checked={alsoRender}
									onCheckedChange={(v) => setAlsoRender(v === true)}
									aria-label="Also render MP4"
								/>
								<span>Also render MP4</span>
							</div>
						</div>

						<fieldset className="flex flex-col gap-2">
							<legend className="mb-1 text-sm text-fg-muted">
								Open exports in
							</legend>
							<RadioGroup
								value={target ?? undefined}
								onValueChange={(v) => setTarget(v as NleTarget)}
							>
								{NLE_LABELS.map(({ value, label }) => (
									<Label
										key={value}
										className="flex items-center gap-2 text-sm"
									>
										<RadioGroupItem value={value} />
										{label}
									</Label>
								))}
							</RadioGroup>
						</fieldset>

						{error !== null && <p className="text-sm text-failed">{error}</p>}

						<DialogFooter>
							<Button variant="ghost" onClick={onClose}>
								Cancel
							</Button>
							<Button
								disabled={target === null || preview === null || busy}
								onClick={() => void runExport()}
							>
								Export
							</Button>
						</DialogFooter>
					</div>
				) : (
					<div className="flex flex-col gap-4">
						<div className="flex flex-col gap-1 font-mono text-sm tabular-nums">
							<span>{done.fcpxml_path}</span>
							<span>{done.srt_path}</span>
							<span>{done.vtt_path}</span>
						</div>
						{postError !== null && (
							<p className="text-sm text-failed">{postError}</p>
						)}
						<DialogFooter>
							<Button variant="ghost" onClick={onClose}>
								Done
							</Button>
							{target === "final_cut" ? (
								<Button
									onClick={() =>
										void openInFcp(done.fcpxml_path).catch((e) =>
											setPostError(e instanceof Error ? e.message : String(e)),
										)
									}
								>
									Open in Final Cut
								</Button>
							) : target === "resolve" &&
								resolveInstalled &&
								preview !== null ? (
								<Button
									onClick={() =>
										void openInResolve(preview.slug, preview.version).catch(
											(e) =>
												setPostError(
													e instanceof Error ? e.message : String(e),
												),
										)
									}
								>
									Open in Resolve
								</Button>
							) : (
								<Button
									onClick={() =>
										void revealTimeline(done.fcpxml_path).catch((e) =>
											setPostError(e instanceof Error ? e.message : String(e)),
										)
									}
								>
									Reveal in Finder
								</Button>
							)}
						</DialogFooter>
					</div>
				)}
			</DialogContent>
		</Dialog>
	);
}
