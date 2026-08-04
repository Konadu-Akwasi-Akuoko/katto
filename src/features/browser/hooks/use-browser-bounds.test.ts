import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useBrowserBounds } from "./use-browser-bounds";

type Rect = { x: number; y: number; width: number; height: number };

/** A host div whose layout rect the test drives, since jsdom has none. */
function hostRef(initial: Rect) {
	const el = document.createElement("div");
	const current = { ...initial };
	el.getBoundingClientRect = () =>
		({
			...current,
			top: current.y,
			left: current.x,
			right: current.x + current.width,
			bottom: current.y + current.height,
			toJSON: () => ({}),
		}) as DOMRect;
	return {
		ref: { current: el },
		move(next: Rect) {
			Object.assign(current, next);
		},
	};
}

function recordBounds(inset = 0) {
	// getCurrentWindow() reads the window label out of the Tauri metadata, which
	// mockIPC alone does not install.
	mockWindows("main");
	const sent = vi.fn();
	mockIPC((cmd, payload) => {
		if (cmd === "browser_set_bounds") {
			sent((payload as { rect: Rect }).rect);
		}
		// The window content view is `inset` logical px taller than the document
		// viewport under FullSizeContentView; the hook derives that difference.
		if (cmd === "plugin:window|inner_size") {
			return { width: 1800, height: (window.innerHeight + inset) * 2 };
		}
		if (cmd === "plugin:window|scale_factor") {
			return 2;
		}
		return null;
	});
	return sent;
}

async function flushFrame() {
	await act(async () => {
		await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
	});
}

describe("useBrowserBounds", () => {
	it("reports the host rect verbatim on mount", () => {
		const sent = recordBounds();
		const host = hostRef({ x: 220, y: 68, width: 900, height: 600 });
		renderHook(() => useBrowserBounds(host.ref));
		expect(sent).toHaveBeenCalledTimes(1);
		expect(sent).toHaveBeenCalledWith({
			x: 220,
			y: 68,
			width: 900,
			height: 600,
		});
	});

	// The regression that buried the toolbar: wry positions child webviews
	// against the window content view, which FullSizeContentView makes taller
	// than the document viewport. Sending a DOM-relative y put the page one
	// inset too high, over the toolbar.
	it("shifts the reported y down by the viewport inset", async () => {
		const sent = recordBounds(32);
		const host = hostRef({ x: 220, y: 68, width: 900, height: 600 });
		renderHook(() => useBrowserBounds(host.ref));
		await act(async () => {
			await Promise.resolve();
			await Promise.resolve();
		});
		expect(sent).toHaveBeenLastCalledWith({
			x: 220,
			y: 100,
			width: 900,
			height: 600,
		});
	});

	// A component test that mocks IPC without the window metadata used to make
	// the inset lookup reject, escaping the effect as an unhandled rejection.
	it("falls back to no correction when the window size is unreadable", async () => {
		const sent = vi.fn();
		mockIPC((cmd, payload) => {
			if (cmd === "browser_set_bounds") {
				sent((payload as { rect: Rect }).rect);
			}
			return null;
		});
		const host = hostRef({ x: 220, y: 68, width: 900, height: 600 });
		renderHook(() => useBrowserBounds(host.ref));
		await act(async () => {
			await Promise.resolve();
			await Promise.resolve();
		});
		expect(sent).toHaveBeenLastCalledWith({
			x: 220,
			y: 68,
			width: 900,
			height: 600,
		});
	});

	it("leaves y alone when the window has no inset", async () => {
		const sent = recordBounds(0);
		const host = hostRef({ x: 220, y: 68, width: 900, height: 600 });
		renderHook(() => useBrowserBounds(host.ref));
		await act(async () => {
			await Promise.resolve();
			await Promise.resolve();
		});
		expect(sent).toHaveBeenCalledTimes(1);
		expect(sent).toHaveBeenLastCalledWith({
			x: 220,
			y: 68,
			width: 900,
			height: 600,
		});
	});

	it("drops a remeasure that would resend an unchanged rect", async () => {
		const sent = recordBounds();
		const host = hostRef({ x: 220, y: 68, width: 900, height: 600 });
		const { result } = renderHook(() => useBrowserBounds(host.ref));
		act(() => result.current());
		await flushFrame();
		expect(sent).toHaveBeenCalledTimes(1);
	});

	it("reports a host that moved without resizing", async () => {
		const sent = recordBounds();
		const host = hostRef({ x: 220, y: 68, width: 900, height: 600 });
		const { result } = renderHook(() => useBrowserBounds(host.ref));
		host.move({ x: 220, y: 96, width: 900, height: 600 });
		act(() => result.current());
		await flushFrame();
		expect(sent).toHaveBeenCalledTimes(2);
		expect(sent).toHaveBeenLastCalledWith({
			x: 220,
			y: 96,
			width: 900,
			height: 600,
		});
	});

	it("coalesces a burst of remeasures into one send", async () => {
		const sent = recordBounds();
		const host = hostRef({ x: 220, y: 68, width: 900, height: 600 });
		const { result } = renderHook(() => useBrowserBounds(host.ref));
		host.move({ x: 220, y: 96, width: 900, height: 600 });
		act(() => {
			result.current();
			result.current();
			result.current();
		});
		await flushFrame();
		expect(sent).toHaveBeenCalledTimes(2);
	});

	it("sends nothing while the host ref is unattached", () => {
		const sent = recordBounds();
		renderHook(() => useBrowserBounds({ current: null }));
		expect(sent).not.toHaveBeenCalled();
	});
});
