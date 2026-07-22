import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal as XTerm } from "@xterm/xterm";
import { useEffect, useRef } from "react";
import { attachSession, resizeSession, writeSession } from "@/lib/ipc/sessions";
import "@xterm/xterm/css/xterm.css";

function cssVar(name: string, fallback: string): string {
	const value = getComputedStyle(document.documentElement)
		.getPropertyValue(name)
		.trim();
	return value === "" ? fallback : value;
}

/**
 * One xterm instance per mounted session. The panel keys this by session id,
 * so switching tabs unmounts/remounts and the backend scrollback ring replays
 * — cheap and always correct. The viewport is the dock's ONLY mono surface
 * and opts out of grain (machine region).
 */
export function Terminal({ sessionId }: { sessionId: string }) {
	const containerRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		const container = containerRef.current;
		if (container === null) return;

		const term = new XTerm({
			fontFamily: cssVar("--mono", "ui-monospace, monospace"),
			fontSize: 12,
			theme: {
				background: cssVar("--term-bg", "#1a1917"),
				foreground: cssVar("--term-fg", "#d9d6d1"),
				cursor: cssVar("--term-fg", "#d9d6d1"),
				selectionBackground: cssVar("--ember", "#c75c28"),
			},
			scrollback: 5000,
		});
		const fit = new FitAddon();
		term.loadAddon(fit);
		try {
			const webgl = new WebglAddon();
			webgl.onContextLoss(() => webgl.dispose());
			term.loadAddon(webgl);
		} catch {
			// WebGL unavailable — xterm falls back to the DOM renderer.
		}
		term.open(container);
		fit.fit();

		void attachSession(sessionId, (bytes) => term.write(bytes)).catch(() => {
			term.write(`\r\nkatto: could not attach to session ${sessionId}\r\n`);
		});
		const dataListener = term.onData((data) => {
			void writeSession(sessionId, data).catch(() => {});
		});
		const resizeListener = term.onResize(({ cols, rows }) => {
			void resizeSession(sessionId, cols, rows).catch(() => {});
		});

		let fitTimer: ReturnType<typeof setTimeout> | undefined;
		const observer = new ResizeObserver(() => {
			clearTimeout(fitTimer);
			fitTimer = setTimeout(() => fit.fit(), 100);
		});
		observer.observe(container);

		return () => {
			observer.disconnect();
			clearTimeout(fitTimer);
			dataListener.dispose();
			resizeListener.dispose();
			term.dispose();
		};
	}, [sessionId]);

	return (
		<div
			ref={containerRef}
			className="h-full min-h-0 overflow-hidden rounded-lg bg-term-bg p-2"
			style={{ backgroundImage: "none" }}
		/>
	);
}
