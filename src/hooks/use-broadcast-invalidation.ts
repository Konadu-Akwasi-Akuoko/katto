import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { onDriveStatusChanged, onEventsAppended, onJobsChanged } from "@/lib/ipc/broadcast";
import { driveKeys } from "@/lib/ipc/drive";
import { eventsKeys } from "@/lib/ipc/events";
import { jobsKeys } from "@/lib/ipc/jobs";

/**
 * Bridge backend broadcasts into TanStack Query. Listeners attach per WebView
 * lifetime; a rebuilt window re-attaches on mount and its queries refetch, so
 * no broadcast is ever load-bearing for initial state.
 */
export function useBroadcastInvalidation(): void {
	const queryClient = useQueryClient();
	useEffect(() => {
		const subscriptions = [
			onEventsAppended(() => {
				void queryClient.invalidateQueries({ queryKey: eventsKeys.all });
			}),
			onJobsChanged(() => {
				void queryClient.invalidateQueries({ queryKey: jobsKeys.all });
			}),
			onDriveStatusChanged(() => {
				void queryClient.invalidateQueries({ queryKey: driveKeys.status });
			}),
		];
		return () => {
			for (const subscription of subscriptions) {
				void subscription.then((unlisten) => unlisten());
			}
		};
	}, [queryClient]);
}
