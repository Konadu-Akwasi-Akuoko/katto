import type {
	DeepLinkOpened,
	DriveStatusChanged,
	SessionStateChanged,
} from "@/lib/ipc/bindings.gen";
import { events } from "@/lib/ipc/bindings.gen";

export type { DeepLinkOpened, DriveStatusChanged, SessionStateChanged };

type Unlisten = () => void;

/** The events log grew — refetch feeds. */
export const onEventsAppended = (callback: () => void): Promise<Unlisten> =>
	events.eventsAppended.listen(() => callback());

/** A job was created or changed state — refetch job lists. */
export const onJobsChanged = (callback: () => void): Promise<Unlisten> =>
	events.jobsChanged.listen(() => callback());

/** An idea row was created or changed (create, update, discard, promote). */
export const onIdeasChanged = (callback: () => void): Promise<Unlisten> =>
	events.ideasChanged.listen(() => callback());

/** A project row was created, mutated (status/dates), or reconciled. */
export const onProjectsChanged = (callback: () => void): Promise<Unlisten> =>
	events.projectsChanged.listen(() => callback());

/** A schedule entry was upserted or deleted — refetch calendar ranges. */
export const onScheduleChanged = (callback: () => void): Promise<Unlisten> =>
	events.scheduleChanged.listen(() => callback());

/** The studio root mounted or disconnected. */
export const onDriveStatusChanged = (
	callback: (status: DriveStatusChanged) => void,
): Promise<Unlisten> =>
	events.driveStatusChanged.listen((event) => callback(event.payload));

/** A camera card was detected and enumerated — refetch the offer query. */
export const onCardDetected = (callback: () => void): Promise<Unlisten> =>
	events.cardDetected.listen(() => callback());

/** The detected card's volume was unmounted — drop the stale offer. */
export const onCardRemoved = (callback: () => void): Promise<Unlisten> =>
	events.cardRemoved.listen(() => callback());

/** The session set changed (spawn/close/reap) — refetch the dock list. */
export const onSessionsChanged = (callback: () => void): Promise<Unlisten> =>
	events.sessionsChanged.listen(() => callback());

/** A session's state changed — refetch the dock list (dots, icon, notes). */
export const onSessionStateChanged = (
	callback: (payload: SessionStateChanged) => void,
): Promise<Unlisten> =>
	events.sessionStateChanged.listen((event) => callback(event.payload));

/** A `katto://` deep link was opened — navigate to its route. */
export const onDeepLinkOpened = (
	callback: (payload: DeepLinkOpened) => void,
): Promise<Unlisten> =>
	events.deepLinkOpened.listen((event) => callback(event.payload));
