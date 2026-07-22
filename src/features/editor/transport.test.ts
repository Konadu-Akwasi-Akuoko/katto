import { describe, expect, it } from "vitest";
import { isEditableTarget, keyToAction } from "@/features/editor/transport";

describe("keyToAction", () => {
	it.each([
		[
			{ key: " ", code: "Space", shiftKey: false, metaKey: false },
			{ kind: "toggle-play" },
		],
		[
			{ key: "k", code: "KeyK", shiftKey: false, metaKey: false },
			{ kind: "stop" },
		],
		[
			{ key: "l", code: "KeyL", shiftKey: false, metaKey: false },
			{ kind: "play-forward" },
		],
		[
			{ key: "j", code: "KeyJ", shiftKey: false, metaKey: false },
			{ kind: "shuttle-back" },
		],
		[
			{
				key: "ArrowRight",
				code: "ArrowRight",
				shiftKey: false,
				metaKey: false,
			},
			{ kind: "step-frames", frames: 1 },
		],
		[
			{ key: "ArrowLeft", code: "ArrowLeft", shiftKey: true, metaKey: false },
			{ kind: "step-frames", frames: -10 },
		],
		[
			{ key: "z", code: "KeyZ", shiftKey: false, metaKey: true },
			{ kind: "undo" },
		],
		[
			{ key: "z", code: "KeyZ", shiftKey: true, metaKey: true },
			{ kind: "redo" },
		],
		[
			{ key: "x", code: "KeyX", shiftKey: false, metaKey: false },
			{ kind: "manual-cut" },
		],
		[
			{ key: "o", code: "KeyO", shiftKey: false, metaKey: false },
			{ kind: "toggle-original" },
		],
	])("maps %o", (e, expected) => expect(keyToAction(e)).toEqual(expected));

	it("returns null for unmapped keys and plain meta chords", () => {
		expect(
			keyToAction({ key: "q", code: "KeyQ", shiftKey: false, metaKey: false }),
		).toBeNull();
		expect(
			keyToAction({ key: "l", code: "KeyL", shiftKey: false, metaKey: true }),
		).toBeNull();
	});

	it("ignores keys while typing in an input", () => {
		expect(isEditableTarget(document.createElement("input"))).toBe(true);
		expect(isEditableTarget(document.createElement("textarea"))).toBe(true);
		expect(isEditableTarget(document.createElement("div"))).toBe(false);
	});
});
