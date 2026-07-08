import { useQuery } from "@tanstack/react-query";
import { getSettings, settingsKeys } from "@/lib/ipc/settings";

/** The app settings object, cached app-wide under `settingsKeys.all`. */
export function useSettings() {
	return useQuery({ queryKey: settingsKeys.all, queryFn: getSettings });
}
