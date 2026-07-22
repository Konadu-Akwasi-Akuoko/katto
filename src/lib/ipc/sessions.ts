import { Channel } from "@tauri-apps/api/core";
import type { NewSession, SessionInfo } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { NewSession, SessionInfo };

export const sessionsKeys = {
	all: ["sessions"] as const,
};

/** Every session the pool knows about, oldest-first. */
export const listSessions = (): Promise<SessionInfo[]> =>
	unwrap(commands.listSessions());

/** Spawn a claude dock session; resolves to its session id. */
export const spawnSession = (task: NewSession): Promise<string> =>
	unwrap(commands.spawnSession(task));

/** Forward xterm keystrokes (already encoded) to the session's PTY. */
export const writeSession = (id: string, data: string): Promise<null> =>
	unwrap(commands.writeSession(id, data));

/** Propagate an xterm resize to the PTY. */
export const resizeSession = (
	id: string,
	cols: number,
	rows: number,
): Promise<null> => unwrap(commands.resizeSession(id, cols, rows));

/** Close a session (user-closed). */
export const closeSession = (id: string): Promise<null> =>
	unwrap(commands.closeSession(id));

/** Report dock visibility/focus for reap exemption + notification suppression. */
export const setDockFocus = (
	open: boolean,
	focusedSession: string | null,
): Promise<null> => unwrap(commands.setDockFocus(open, focusedSession));

/**
 * Attach to a session's output stream. The backend replays scrollback as the
 * first batch, then live output follows in 16 ms / 16 KB batches.
 */
export const attachSession = (
	id: string,
	onData: (bytes: Uint8Array) => void,
): Promise<null> => {
	const channel = new Channel<number[]>();
	channel.onmessage = (data) => onData(new Uint8Array(data));
	return unwrap(commands.attachSession(id, channel));
};
