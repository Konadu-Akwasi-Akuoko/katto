import { events } from "@/lib/ipc/bindings.gen";
import type { DriveStatusChanged } from "@/lib/ipc/bindings.gen";

export type { DriveStatusChanged };

type Unlisten = () => void;

/** The events log grew — refetch feeds. */
export const onEventsAppended = (callback: () => void): Promise<Unlisten> =>
	events.eventsAppended.listen(() => callback());

/** A job was created or changed state — refetch job lists. */
export const onJobsChanged = (callback: () => void): Promise<Unlisten> =>
	events.jobsChanged.listen(() => callback());

/** The studio root mounted or disconnected. */
export const onDriveStatusChanged = (
	callback: (status: DriveStatusChanged) => void,
): Promise<Unlisten> => events.driveStatusChanged.listen((event) => callback(event.payload));
