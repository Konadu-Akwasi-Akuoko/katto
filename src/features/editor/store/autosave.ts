import { toWireEdits } from "@/features/editor/model/wire";
import type { EditorStore } from "@/features/editor/store/editor-store";
import { documentOf, historyOf } from "@/features/editor/store/editor-store";
import type { Edits_Deserialize, Rational } from "@/lib/ipc/bindings.gen";

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
}): { flushNow(): Promise<void>; dispose(): void; state(): AutosaveState } {
	const debounceMs = opts.debounceMs ?? DEFAULT_DEBOUNCE_MS;
	let state: AutosaveState = "idle";
	let timer: ReturnType<typeof setTimeout> | null = null;
	let running: Promise<void> | null = null;

	const payload = (): Edits_Deserialize =>
		toWireEdits(
			documentOf(opts.store.getState()),
			historyOf(opts.store),
			opts.fps,
		);

	const run = async (): Promise<void> => {
		state = "saving";
		try {
			await opts.save(payload());
			state = "idle";
		} catch {
			try {
				await opts.save(payload());
				state = "idle";
			} catch (retryError) {
				state = "paused-error";
				opts.onPaused(
					retryError instanceof Error ? retryError.message : String(retryError),
				);
			}
		}
	};

	const schedule = (): void => {
		if (state === "paused-error") return;
		state = "pending";
		if (timer !== null) clearTimeout(timer);
		timer = setTimeout(() => {
			timer = null;
			running = run();
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
			if (running !== null) await running;
			running = run();
			await running;
		},
		dispose: () => {
			if (timer !== null) clearTimeout(timer);
			unsubscribeStore();
			unsubscribeTemporal();
		},
	};
}
