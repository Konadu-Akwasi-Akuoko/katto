// Pure transport key map (PRD row "Kept-only playback", exact bindings).

export type TransportAction =
	| { kind: "toggle-play" }
	| { kind: "stop" }
	| { kind: "play-forward" } // L; stacking bumps rate to 2
	| { kind: "shuttle-back" } // J; reverse-ish scrub
	| { kind: "step-frames"; frames: number } // arrows = ±1, Shift = ±10
	| { kind: "undo" }
	| { kind: "redo" }
	| { kind: "manual-cut" } // X / Delete with a live selection
	| { kind: "toggle-original" }; // O

/** Keyboard shortcuts must not fire while typing in a field. */
export function isEditableTarget(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	return (
		target instanceof HTMLInputElement ||
		target instanceof HTMLTextAreaElement ||
		target.isContentEditable === true
	);
}

/** Map a keyboard event to its transport action; null when unmapped. */
export function keyToAction(
	e: Pick<KeyboardEvent, "key" | "code" | "shiftKey" | "metaKey">,
): TransportAction | null {
	if (e.metaKey) {
		if (e.code === "KeyZ") {
			return e.shiftKey ? { kind: "redo" } : { kind: "undo" };
		}
		return null;
	}
	switch (e.code) {
		case "Space":
			return { kind: "toggle-play" };
		case "KeyK":
			return { kind: "stop" };
		case "KeyL":
			return { kind: "play-forward" };
		case "KeyJ":
			return { kind: "shuttle-back" };
		case "ArrowRight":
			return { kind: "step-frames", frames: e.shiftKey ? 10 : 1 };
		case "ArrowLeft":
			return { kind: "step-frames", frames: e.shiftKey ? -10 : -1 };
		case "KeyX":
		case "Delete":
			return { kind: "manual-cut" };
		case "KeyO":
			return { kind: "toggle-original" };
		default:
			return null;
	}
}
