import { render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Terminal } from "./terminal";

const xtermInstances: MockXTerm[] = [];

class MockXTerm {
	options: unknown;
	open = vi.fn();
	write = vi.fn();
	loadAddon = vi.fn();
	dispose = vi.fn();
	dataHandler: ((data: string) => void) | null = null;
	resizeHandler: ((size: { cols: number; rows: number }) => void) | null = null;

	constructor(options: unknown) {
		this.options = options;
		xtermInstances.push(this);
	}

	onData(cb: (data: string) => void) {
		this.dataHandler = cb;
		return { dispose: vi.fn() };
	}

	onResize(cb: (size: { cols: number; rows: number }) => void) {
		this.resizeHandler = cb;
		return { dispose: vi.fn() };
	}
}

vi.mock("@xterm/xterm", () => ({
	Terminal: function (this: unknown, options: unknown) {
		return new MockXTerm(options);
	},
}));
vi.mock("@xterm/addon-fit", () => ({
	FitAddon: class {
		fit = vi.fn();
	},
}));
vi.mock("@xterm/addon-webgl", () => ({
	WebglAddon: class {
		onContextLoss = vi.fn();
		dispose = vi.fn();
	},
}));

const attachCallbacks = new Map<string, (bytes: Uint8Array) => void>();
const { attachSession, writeSession, resizeSession } = vi.hoisted(() => ({
	attachSession: vi.fn(
		(id: string, onData: (bytes: Uint8Array) => void): Promise<null> => {
			attachCallbacks.set(id, onData);
			return Promise.resolve(null);
		},
	),
	writeSession: vi.fn(() => Promise.resolve(null)),
	resizeSession: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@/lib/ipc/sessions", () => ({
	attachSession,
	writeSession,
	resizeSession,
}));

describe("Terminal", () => {
	beforeEach(() => {
		xtermInstances.length = 0;
		attachCallbacks.clear();
		vi.clearAllMocks();
	});

	it("opens xterm, attaches, and forwards stream bytes in order", async () => {
		render(<Terminal sessionId="s1" />);
		const term = xtermInstances[0];
		if (!term) throw new Error("xterm never constructed");
		expect(term.open).toHaveBeenCalled();
		await vi.waitFor(() => {
			expect(attachSession).toHaveBeenCalledWith("s1", expect.any(Function));
		});
		const onData = attachCallbacks.get("s1");
		if (!onData) throw new Error("attach callback missing");
		onData(new Uint8Array([104, 105]));
		onData(new Uint8Array([33]));
		onData(new Uint8Array([10]));
		expect(term.write.mock.calls.map((c) => c[0])).toEqual([
			new Uint8Array([104, 105]),
			new Uint8Array([33]),
			new Uint8Array([10]),
		]);
	});

	it("forwards keystrokes to writeSession", async () => {
		render(<Terminal sessionId="s1" />);
		const term = xtermInstances[0];
		if (!term?.dataHandler) throw new Error("onData never registered");
		term.dataHandler("ls\n");
		expect(writeSession).toHaveBeenCalledWith("s1", "ls\n");
	});

	it("propagates xterm resizes to the backend", async () => {
		render(<Terminal sessionId="s1" />);
		const term = xtermInstances[0];
		if (!term?.resizeHandler) throw new Error("onResize never registered");
		term.resizeHandler({ cols: 96, rows: 40 });
		expect(resizeSession).toHaveBeenCalledWith("s1", 96, 40);
	});

	it("disposes the terminal on unmount", () => {
		const { unmount } = render(<Terminal sessionId="s1" />);
		unmount();
		const term = xtermInstances[0];
		if (!term) throw new Error("xterm never constructed");
		expect(term.dispose).toHaveBeenCalled();
	});
});
