import { useQueryClient } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { useEffect } from "react";
import {
	onBrowserStateChanged,
	onDriveStatusChanged,
	onEventsAppended,
	onIdeasChanged,
	onJobsChanged,
	onProjectsChanged,
	onScheduleChanged,
	onSessionStateChanged,
	onSessionsChanged,
	onStudioImportFinished,
	onThumbnailsChanged,
	onVfxRenderLanded,
} from "@/lib/ipc/broadcast";
import { browserKeys } from "@/lib/ipc/browser";
import { driveKeys } from "@/lib/ipc/drive";
import { eventsKeys } from "@/lib/ipc/events";
import { ideasKeys } from "@/lib/ipc/ideas";
import { jobsKeys } from "@/lib/ipc/jobs";
import { projectsKeys } from "@/lib/ipc/projects";
import { scheduleKeys } from "@/lib/ipc/schedule";
import { sessionsKeys } from "@/lib/ipc/sessions";
import { thumbKeys } from "@/lib/ipc/thumbnails";
import { vfxKeys } from "@/lib/ipc/vfx";

/**
 * Bridge backend broadcasts into TanStack Query. Listeners attach per WebView
 * lifetime; a rebuilt window re-attaches on mount and its queries refetch, so
 * no broadcast is ever load-bearing for initial state.
 */
export function useBroadcastInvalidation(): void {
	const queryClient = useQueryClient();
	useEffect(() => {
		// In a plain browser (design QA against the vite server) there is no
		// IPC bridge; listen() would throw synchronously and unmount the app.
		if (!isTauri()) return;
		const subscriptions = [
			onEventsAppended(() => {
				void queryClient.invalidateQueries({ queryKey: eventsKeys.all });
			}),
			onJobsChanged(() => {
				void queryClient.invalidateQueries({ queryKey: jobsKeys.all });
			}),
			onIdeasChanged(() => {
				void queryClient.invalidateQueries({ queryKey: ideasKeys.all });
			}),
			onProjectsChanged(() => {
				void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
			}),
			onScheduleChanged(() => {
				void queryClient.invalidateQueries({ queryKey: scheduleKeys.all });
			}),
			onDriveStatusChanged(() => {
				void queryClient.invalidateQueries({ queryKey: driveKeys.status });
			}),
			onSessionsChanged(() => {
				void queryClient.invalidateQueries({ queryKey: sessionsKeys.all });
			}),
			onSessionStateChanged(() => {
				void queryClient.invalidateQueries({ queryKey: sessionsKeys.all });
			}),
			onVfxRenderLanded((payload) => {
				void queryClient.invalidateQueries({
					queryKey: vfxKeys.byProject(payload.slug),
				});
			}),
			onBrowserStateChanged(() => {
				void queryClient.invalidateQueries({ queryKey: browserKeys.state });
			}),
			onStudioImportFinished(() => {
				void queryClient.invalidateQueries({ queryKey: ideasKeys.all });
			}),
			onThumbnailsChanged((payload) => {
				void queryClient.invalidateQueries({
					queryKey: thumbKeys.latest(payload.slug),
				});
				void queryClient.invalidateQueries({ queryKey: thumbKeys.all });
			}),
		];
		return () => {
			for (const subscription of subscriptions) {
				void subscription.then((unlisten) => unlisten());
			}
		};
	}, [queryClient]);
}
