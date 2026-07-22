import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createAutosave } from "@/features/editor/store/autosave";
import { createEditorStore } from "@/features/editor/store/editor-store";
import type { Cuts } from "@/lib/ipc/pipeline";

const cuts: Cuts = {
	source_duration_secs: 20,
	cuts: [
		{ start: 1, end: 2, reason: "filler", excerpt: "" },
		{ start: 4, end: 5, reason: "stutter", excerpt: "" },
	],
	discretionary: [],
	flags: [],
	total_cut_secs: 2,
};

const fps = { num: 25, den: 1 };

function makeStore() {
	return createEditorStore({
		document: {
			toggledOff: [],
			appliedDiscretionary: [],
			manualCuts: [],
			boundaryAdjustments: [],
		},
		history: { past: [], future: [] },
		cuts,
		tokens: [],
		fps,
	});
}

beforeEach(() => {
	vi.useFakeTimers();
});
afterEach(() => {
	vi.useRealTimers();
});

describe("autosave", () => {
	it("coalesces edits into one save 200ms after the last change", async () => {
		const store = makeStore();
		const save = vi.fn().mockResolvedValue(null);
		const { dispose } = createAutosave({ store, fps, save, onPaused: vi.fn() });
		store.getState().toggleCut(0);
		store.getState().toggleCut(1);
		await vi.advanceTimersByTimeAsync(199);
		expect(save).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		expect(save).toHaveBeenCalledTimes(1);
		expect(save.mock.calls[0]?.[0]?.history).toBeDefined();
		expect(save.mock.calls[0]?.[0]?.toggled_off).toEqual([0, 1]);
		dispose();
	});

	it("save failure retries once then pauses with a banner", async () => {
		const store = makeStore();
		const save = vi.fn().mockRejectedValue(new Error("disk full"));
		const onPaused = vi.fn();
		const autosave = createAutosave({ store, fps, save, onPaused });
		store.getState().toggleCut(0);
		await vi.advanceTimersByTimeAsync(200);
		await vi.runAllTimersAsync();
		expect(save).toHaveBeenCalledTimes(2);
		expect(onPaused).toHaveBeenCalledOnce();
		expect(autosave.state()).toBe("paused-error");
		autosave.dispose();
	});

	it("a successful flushNow resumes after a paused error", async () => {
		const store = makeStore();
		const save = vi
			.fn()
			.mockRejectedValueOnce(new Error("disk full"))
			.mockRejectedValueOnce(new Error("disk full"))
			.mockResolvedValue(null);
		const onPaused = vi.fn();
		const autosave = createAutosave({ store, fps, save, onPaused });
		store.getState().toggleCut(0);
		await vi.advanceTimersByTimeAsync(200);
		await vi.runAllTimersAsync();
		expect(autosave.state()).toBe("paused-error");
		await autosave.flushNow();
		expect(autosave.state()).toBe("idle");
		autosave.dispose();
	});

	it("an edit during an in-flight save is never reported idle", async () => {
		const store = makeStore();
		const pending: Array<(v: null) => void> = [];
		const save = vi
			.fn()
			.mockImplementation(
				() => new Promise<null>((resolve) => pending.push(resolve)),
			);
		const autosave = createAutosave({ store, fps, save, onPaused: vi.fn() });
		store.getState().toggleCut(0);
		await vi.advanceTimersByTimeAsync(200);
		expect(autosave.state()).toBe("saving");
		// Edit lands while the first save is still in flight.
		store.getState().toggleCut(1);
		for (const resolve of pending.splice(0)) {
			resolve(null);
		}
		await Promise.resolve();
		await Promise.resolve();
		// The resolved save must not clobber the newer pending edit to idle —
		// the close guard would skip its flush and silently drop the edit.
		expect(autosave.state()).not.toBe("idle");
		await vi.advanceTimersByTimeAsync(200);
		for (const resolve of pending.splice(0)) {
			resolve(null);
		}
		await vi.runAllTimersAsync();
		expect(save).toHaveBeenCalledTimes(2);
		expect(autosave.state()).toBe("idle");
		autosave.dispose();
	});

	it("the next edit genuinely retries after a paused error", async () => {
		const store = makeStore();
		const save = vi
			.fn()
			.mockRejectedValueOnce(new Error("disk full"))
			.mockRejectedValueOnce(new Error("disk full"))
			.mockResolvedValue(null);
		const onResumed = vi.fn();
		const autosave = createAutosave({
			store,
			fps,
			save,
			onPaused: vi.fn(),
			onResumed,
		});
		store.getState().toggleCut(0);
		await vi.advanceTimersByTimeAsync(200);
		await vi.runAllTimersAsync();
		expect(autosave.state()).toBe("paused-error");
		store.getState().toggleCut(1);
		await vi.advanceTimersByTimeAsync(200);
		await vi.runAllTimersAsync();
		expect(save).toHaveBeenCalledTimes(3);
		expect(autosave.state()).toBe("idle");
		expect(onResumed).toHaveBeenCalledOnce();
		autosave.dispose();
	});

	it("undo and redo also persist", async () => {
		const store = makeStore();
		const save = vi.fn().mockResolvedValue(null);
		const { dispose } = createAutosave({ store, fps, save, onPaused: vi.fn() });
		store.getState().toggleCut(0);
		await vi.advanceTimersByTimeAsync(200);
		await vi.runAllTimersAsync();
		save.mockClear();
		store.temporal.getState().undo();
		await vi.advanceTimersByTimeAsync(200);
		await vi.runAllTimersAsync();
		expect(save).toHaveBeenCalled();
		expect(save.mock.calls[0]?.[0]?.toggled_off).toEqual([]);
		dispose();
	});
});
