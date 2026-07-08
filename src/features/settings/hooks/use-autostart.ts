import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { autostartKeys, getAutostart, setAutostart } from "@/lib/ipc/shell";

/** OS launch-at-login state and its toggle mutation. */
export function useAutostart() {
	const queryClient = useQueryClient();
	const state = useQuery({ queryKey: autostartKeys.all, queryFn: getAutostart });
	const toggle = useMutation({
		mutationFn: setAutostart,
		onSuccess: () =>
			queryClient.invalidateQueries({ queryKey: autostartKeys.all }),
	});
	return { state, toggle };
}
