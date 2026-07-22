import { useQuery } from "@tanstack/react-query";
import { listSessions, sessionsKeys } from "@/lib/ipc/sessions";

/** The dock's session list; invalidated by session broadcasts. */
export function useSessions() {
	return useQuery({
		queryKey: sessionsKeys.all,
		queryFn: listSessions,
	});
}
