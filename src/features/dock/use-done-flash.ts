import { useEffect, useRef, useState } from "react";
import type { SessionInfo } from "@/lib/ipc/sessions";

/**
 * True for ~3s after a previously-running session settles (idle/closed).
 * The timer lives in a ref so an unrelated sessions update mid-flash cannot
 * cancel it and strand the check lit.
 */
export function useDoneFlash(sessions: SessionInfo[]): boolean {
	const prevRunning = useRef<Set<string>>(new Set());
	const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
	const [flash, setFlash] = useState(false);

	useEffect(() => {
		const running = new Set(
			sessions.filter((s) => s.state.kind === "running").map((s) => s.id),
		);
		const settled = [...prevRunning.current].some((id) => {
			const session = sessions.find((s) => s.id === id);
			return (
				session !== undefined &&
				(session.state.kind === "idle" || session.state.kind === "closed")
			);
		});
		prevRunning.current = running;
		if (settled) {
			setFlash(true);
			clearTimeout(timer.current);
			timer.current = setTimeout(() => setFlash(false), 3000);
		}
	}, [sessions]);

	useEffect(() => () => clearTimeout(timer.current), []);

	return flash;
}
