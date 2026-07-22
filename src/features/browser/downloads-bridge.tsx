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
					id: `${payload.project}/${payload.dest_rel}`,
					filename: payload.filename,
					status: "filed",
					project: payload.project,
					destRel: payload.dest_rel,
				});
				toast.success(
					`Filed ${payload.filename} → ${payload.project} · ${payload.dest_rel}`,
				);
			}),
			onDownloadFallback((payload) => {
				upsert({
					id: `fallback-${payload.filename}`,
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
				upsert({
					id: `failed-${payload.filename}`,
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
				setNeedsProject({
					id: payload.download_id,
					filename: payload.filename,
				});
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
