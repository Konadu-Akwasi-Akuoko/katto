import { isTauri } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { toast } from "sonner";
import {
	onDownloadFailed,
	onDownloadFallback,
	onDownloadFiled,
	onDownloadNeedsProject,
} from "@/lib/ipc/broadcast";
import { useDownloadsStore } from "@/stores/downloads";
import { useUiStore } from "@/stores/ui";

// fallback/failed broadcasts carry no id, only a filename; a counter keeps
// re-downloads of the same file from silently overwriting the prior row
let rowSeq = 0;

/**
 * Bridges download broadcasts into the downloads store + toasts. Mounted in
 * app.tsx next to BroadcastBridge so filing feedback fires on any surface,
 * not just while the browser is open.
 */
export function DownloadsBridge() {
	useEffect(() => {
		if (!isTauri()) return;
		const { upsert, setNeedsProject } = useDownloadsStore.getState();
		const subscriptions = [
			onDownloadFiled((payload) => {
				upsert({
					id: payload.download_id,
					filename: payload.filename,
					status: "filed",
					project: payload.project,
					destRel: payload.dest_rel,
				});
				toast.success(
					`Filed ${payload.filename} → ${payload.project}/${payload.dest_rel}`,
				);
			}),
			onDownloadFallback((payload) => {
				rowSeq += 1;
				upsert({
					id: `fallback-${rowSeq}-${payload.filename}`,
					filename: payload.filename,
					status: "fallback",
				});
				toast.warning(
					"Saved to Downloads — katto couldn't intercept this one",
					{
						description: payload.filename,
						duration: Number.POSITIVE_INFINITY,
						closeButton: true,
					},
				);
			}),
			onDownloadFailed((payload) => {
				rowSeq += 1;
				upsert({
					id: `failed-${rowSeq}-${payload.filename}`,
					filename: payload.filename,
					status: "failed",
				});
				toast.error("Download failed", { description: payload.filename });
			}),
			onDownloadNeedsProject((payload) => {
				upsert({
					id: payload.download_id,
					filename: payload.filename,
					status: "needs-project",
				});
				// never steal the sheet from a pick in progress — later parks
				// stay reachable from their popover rows
				const { needsProject } = useDownloadsStore.getState();
				if (needsProject === null) {
					setNeedsProject({
						id: payload.download_id,
						filename: payload.filename,
					});
				}
				// the sheet only renders inside the browser surface — from any
				// other surface the pick must announce itself
				if (useUiStore.getState().surface !== "browser") {
					toast.info("Download waiting for a project pick", {
						description: payload.filename,
						action: {
							label: "Choose",
							onClick: () => useUiStore.getState().setSurface("browser"),
						},
					});
				}
			}),
		];
		return () => {
			for (const subscription of subscriptions) {
				void subscription.then((unlisten) => unlisten());
			}
		};
	}, []);
	return null;
}
