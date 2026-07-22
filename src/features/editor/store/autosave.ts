import { toWireEdits } from "@/features/editor/model/wire";
import type { EditorStore } from "@/features/editor/store/editor-store";
import { documentOf, historyOf } from "@/features/editor/store/editor-store";
import type { Edits_Deserialize, Rational } from "@/lib/ipc/editor";

export type SaveFn = (edits: Edits_Deserialize) => Promise<unknown>;
export type AutosaveState = "idle" | "pending" | "saving" | "paused-error";

const DEFAULT_DEBOUNCE_MS = 200;

/**
 * The debounced auto-save controller — the ONLY interactive-path bridge call.
 * Subscribes to the document store AND its temporal store (undo/redo must
 * persist too); a failed save retries once, then pauses with a banner until a
 * successful flushNow (the close guard also uses flushNow).
 */
export function createAutosave(opts: {
	store: EditorStore;
	fps: Rational;
	save: SaveFn;
	debounceMs?: number;
	onPaused(message: string): void;
	/** A save succeeded after a pause was reported — clear the banner. */
	onResumed?(): void;
}): { flushNow(): Promise<void>; dispose(): void; state(): AutosaveState } {
	const debounceMs = opts.debounceMs ?? DEFAULT_DEBOUNCE_MS;
	let state: AutosaveState = "idle";
	let timer: ReturnType<typeof setTimeout> | null = null;
	let running: Promise<void> | null = null;
	let pauseReported = false;

	const payload = (): Edits_Deserialize =>
		toWireEdits(
			documentOf(opts.store.getState()),
			historyOf(opts.store),
			opts.fps,
		);

	// A resolved save only reports idle when nothing newer is waiting: an
	// edit made during the in-flight save re-armed the timer, and its payload
	// is NOT in what just persisted — clobbering it to idle would let the
	// close guard skip the final flush and silently drop the edit.
	const settle = (): void => {
		state = timer !== null ? "pending" : "idle";
		if (pauseReported) {
			pauseReported = false;
			opts.onResumed?.();
		}
	};

	const run = async (): Promise<void> => {
		state = "saving";
		try {
			await opts.save(payload());
			settle();
		} catch {
			try {
				await opts.save(payload());
				settle();
			} catch (retryError) {
				state = "paused-error";
				pauseReported = true;
				opts.onPaused(
					retryError instanceof Error ? retryError.message : String(retryError),
				);
			}
		}
	};

	// Saves are strictly sequential: a new run chains behind any in-flight one
	// (two concurrent atomic writes to edits.json could land out of order).
	const runChained = (): Promise<void> => {
		const next = (running ?? Promise.resolve()).then(run);
		running = next;
		return next;
	};

	const schedule = (): void => {
		// A paused error is not a dead end — the banner promises the next
		// edit retries, so scheduling always proceeds.
		state = "pending";
		if (timer !== null) clearTimeout(timer);
		timer = setTimeout(() => {
			timer = null;
			void runChained();
		}, debounceMs);
	};

	const unsubscribeStore = opts.store.subscribe(schedule);
	const unsubscribeTemporal = opts.store.temporal.subscribe(schedule);

	return {
		state: () => state,
		flushNow: async () => {
			if (timer !== null) {
				clearTimeout(timer);
				timer = null;
			}
			await runChained();
		},
		dispose: () => {
			if (timer !== null) clearTimeout(timer);
			unsubscribeStore();
			unsubscribeTemporal();
		},
	};
}
