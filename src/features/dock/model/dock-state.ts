import type { SessionInfo } from "@/lib/ipc/bindings.gen";

export type DockIconState = "idle" | "running" | "needs-input";

/**
 * The sidebar Claude icon's state, derived from every session: any
 * needs-input wins (the owner is being waited on), then any running;
 * terminal and idle sessions don't animate.
 */
export function deriveDockIconState(sessions: SessionInfo[]): DockIconState {
	if (sessions.some((s) => s.state.kind === "needs_input")) {
		return "needs-input";
	}
	if (sessions.some((s) => s.state.kind === "running")) {
		return "running";
	}
	return "idle";
}

/** The muted note under a tab label, or null for live sessions. */
export function tabNote(session: SessionInfo): string | null {
	const { state } = session;
	if (state.kind === "closed" && state.reason === "idle_reaped") {
		return "closed after idle";
	}
	if (state.kind === "failed") {
		return state.error;
	}
	return null;
}
