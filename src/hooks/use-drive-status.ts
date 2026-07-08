import { useQuery } from "@tanstack/react-query";
import { driveKeys, getDriveStatus } from "@/lib/ipc/drive";

/** Studio-root reachability, cached under `driveKeys.status` and refreshed by
 * the drive-status broadcast. */
export function useDriveStatus() {
	return useQuery({ queryKey: driveKeys.status, queryFn: getDriveStatus });
}
