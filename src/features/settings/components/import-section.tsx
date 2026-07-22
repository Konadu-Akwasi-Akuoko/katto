import { useMutation } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Callout } from "@/components/ui/callout";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { onStudioImportFinished } from "@/lib/ipc/broadcast";
import type { ImportReport } from "@/lib/ipc/import-studio";
import { importStudioDb } from "@/lib/ipc/import-studio";
import { IpcError } from "@/lib/ipc/result";
import type { Settings } from "@/lib/ipc/settings";

const DEFAULT_DB_PATH = "~/Projects/WebDev/hyper-frames/tools/studio/studio.db";

function formatLastRun(iso: string): string {
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return iso;
	return date.toLocaleString(undefined, {
		day: "2-digit",
		month: "short",
		year: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	});
}

function ReportCounts({ report }: { report: ImportReport }) {
	const skippedLabel = report.warnings.length === 0 ? "unchanged" : "skipped";
	return (
		<div className="flex gap-4 text-sm tabular-nums">
			<span>
				<span className="text-fg">{report.imported}</span>{" "}
				<span className="text-fg-muted">imported</span>
			</span>
			<span>
				<span className="text-fg">{report.updated}</span>{" "}
				<span className="text-fg-muted">updated</span>
			</span>
			<span>
				<span className="text-fg">{report.skipped}</span>{" "}
				<span className="text-fg-muted">{skippedLabel}</span>
			</span>
		</div>
	);
}

/**
 * The one-time studio.db import wizard: dry-run preview, then an apply that
 * runs as a job and reports back over the broadcast. Owns its error surface
 * inline (Callout), not toasts.
 */
export function ImportSection({ settings }: { settings: Settings }) {
	const [path, setPath] = useState(DEFAULT_DB_PATH);
	const [preview, setPreview] = useState<ImportReport | null>(null);
	const [finished, setFinished] = useState<ImportReport | null>(null);
	const [applying, setApplying] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		if (!isTauri()) return;
		const subscription = onStudioImportFinished((payload) => {
			setFinished(payload.report);
			setApplying(false);
		});
		return () => {
			void subscription.then((unlisten) => unlisten());
		};
	}, []);

	const dryRun = useMutation({
		mutationFn: () => importStudioDb(path, true),
		onMutate: () => {
			setError(null);
			setFinished(null);
		},
		onSuccess: (outcome) => {
			if (outcome.kind === "preview") setPreview(outcome.report);
		},
		onError: (err) =>
			setError(err instanceof IpcError ? err.message : String(err)),
	});
	const apply = useMutation({
		mutationFn: () => importStudioDb(path, false),
		onMutate: () => {
			setError(null);
			setApplying(true);
		},
		onSuccess: (outcome) => {
			// stays pending until StudioImportFinished delivers the report
			if (outcome.kind === "preview") setApplying(false);
		},
		onError: (err) => {
			setApplying(false);
			setError(err instanceof IpcError ? err.message : String(err));
		},
	});

	return (
		<section className="flex flex-col gap-4">
			<h2 className="font-serif text-lg font-semibold">
				Import from studio.db
			</h2>
			<p className="text-sm text-fg-muted">
				Pulls the old planner's ideas in once — statuses mapped, promoted slugs
				kept verbatim. Safe to re-run; existing ideas update by id.
			</p>
			{settings.studio_import_last_run !== null && (
				<p className="text-xs text-fg-faint">
					Last imported {formatLastRun(settings.studio_import_last_run)}.
				</p>
			)}
			<div className="flex items-center gap-2">
				<Label htmlFor="studio-db-path" className="sr-only">
					studio.db path
				</Label>
				<Input
					id="studio-db-path"
					value={path}
					onChange={(event) => setPath(event.target.value)}
					spellCheck={false}
					className="min-w-0 flex-1 cursor-text font-mono text-xs"
				/>
				<Button
					variant="secondary"
					size="sm"
					disabled={dryRun.isPending}
					onClick={() => dryRun.mutate()}
				>
					Dry run
				</Button>
			</div>
			{error !== null && <Callout>{error}</Callout>}
			{finished !== null ? (
				<Callout className="border-done/40 bg-done/10">
					<div className="flex flex-col gap-1.5">
						<span>Imported.</span>
						<ReportCounts report={finished} />
					</div>
				</Callout>
			) : (
				preview !== null && (
					<div className="flex flex-col gap-2">
						<ReportCounts report={preview} />
						{preview.warnings.length > 0 && (
							<ul className="flex flex-col gap-0.5 text-xs text-fg-faint">
								{preview.warnings.map((warning) => (
									<li key={warning}>{warning}</li>
								))}
							</ul>
						)}
						<div>
							<Button
								size="sm"
								disabled={preview.imported === 0 || applying}
								onClick={() => apply.mutate()}
							>
								{applying ? "Importing…" : `Import ${preview.imported} ideas`}
							</Button>
						</div>
					</div>
				)
			)}
		</section>
	);
}
