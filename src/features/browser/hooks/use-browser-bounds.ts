import { getCurrentWindow } from "@tauri-apps/api/window";
import {
	type RefObject,
	useCallback,
	useEffect,
	useLayoutEffect,
	useRef,
} from "react";
import type { BrowserRect } from "@/lib/ipc/browser";
import { browserSetBounds } from "@/lib/ipc/browser";

/**
 * How far the DOM viewport's top sits below the window content area's top.
 *
 * Tauri gives every macOS window `NSWindowStyleMask::FullSizeContentView`, so
 * the content view spans the whole window frame including the titlebar strip,
 * while WKWebView lays the document out below it. Child webviews are positioned
 * against the content view, so a DOM-relative `y` lands one inset too high —
 * measured at 32 logical px on the owner's machine (window content 869,
 * `innerHeight` 837).
 *
 * Derived, never hardcoded: it collapses to 0 when the two origins coincide
 * (fullscreen, a future titlebar style, another platform), so the correction is
 * a no-op rather than a new bug wherever the inset does not exist.
 *
 * `window.screenY` looks like it should answer this and does not — WKWebView
 * reports the screen height there, not the viewport origin.
 */
async function viewportInset(): Promise<number> {
	try {
		const win = getCurrentWindow();
		const [inner, scale] = await Promise.all([
			win.innerSize(),
			win.scaleFactor(),
		]);
		return Math.max(0, inner.height / scale - window.innerHeight);
	} catch {
		// An unreadable window size degrades to "no correction" — the placement a
		// platform without the inset gets anyway. Rejecting instead escapes the
		// effect as an unhandled rejection, and the only thing lost by falling
		// back is the position adjustment it was trying to make.
		return 0;
	}
}

function same(a: BrowserRect | null, b: BrowserRect): boolean {
	return (
		a !== null &&
		a.x === b.x &&
		a.y === b.y &&
		a.width === b.width &&
		a.height === b.height
	);
}

/**
 * Report the browser content-host rect to the webview host, in the coordinate
 * space wry positions child webviews in: the window's content view. That is
 * `getBoundingClientRect()` plus [`viewportInset`].
 *
 * Returns `remeasure`: call it whenever layout may have moved the host without
 * resizing it — a `ResizeObserver` only fires on size change. Sends are
 * deduped, so calling it liberally costs nothing.
 */
export function useBrowserBounds(
	ref: RefObject<HTMLElement | null>,
): () => void {
	const sent = useRef<BrowserRect | null>(null);
	const inset = useRef(0);
	const frame = useRef<number | null>(null);

	const report = useCallback(() => {
		const el = ref.current;
		if (el === null) return;
		const rect = el.getBoundingClientRect();
		const next: BrowserRect = {
			x: rect.x,
			y: rect.y + inset.current,
			width: rect.width,
			height: rect.height,
		};
		if (same(sent.current, next)) return;
		sent.current = next;
		void browserSetBounds(next);
	}, [ref]);

	const schedule = useCallback(() => {
		if (frame.current !== null) return;
		frame.current = requestAnimationFrame(() => {
			frame.current = null;
			report();
		});
	}, [report]);

	useLayoutEffect(() => {
		const el = ref.current;
		if (el === null) return;
		// synchronous: bounds must land before the first browser_set_visible, or
		// the host refuses to materialise the webview and the page never paints.
		// The inset is not known yet — the effect below corrects this first send.
		report();

		const observer = new ResizeObserver(report);
		observer.observe(el);
		window.addEventListener("resize", schedule);

		return () => {
			observer.disconnect();
			window.removeEventListener("resize", schedule);
			if (frame.current !== null) cancelAnimationFrame(frame.current);
			frame.current = null;
		};
	}, [ref, report, schedule]);

	// The inset needs an async round trip for the window's own size, so it lands
	// a beat after the first report. Re-measure on resize too: entering
	// fullscreen drops the titlebar and takes the inset to 0 with it.
	useEffect(() => {
		let live = true;
		const refresh = () => {
			void viewportInset().then((next) => {
				if (!live || next === inset.current) return;
				inset.current = next;
				report();
			});
		};
		refresh();
		window.addEventListener("resize", refresh);
		return () => {
			live = false;
			window.removeEventListener("resize", refresh);
		};
	}, [report]);

	return schedule;
}
