import type { DriveStatus } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { DriveStatus };

export const driveKeys = {
	status: ["drive", "status"] as const,
};

/** Snapshot of studio-root reachability; broadcasts keep the cache fresh. */
export const getDriveStatus = (): Promise<DriveStatus> =>
	unwrap(commands.getDriveStatus());
