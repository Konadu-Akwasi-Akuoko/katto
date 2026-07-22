import { describe, expect, it } from "vitest";
import {
	acceleratorFromKeyboardEvent,
	displayGlyphs,
} from "@/features/settings/model/accelerator";

const ev = (
	code: string,
	mods: Partial<{
		metaKey: boolean;
		ctrlKey: boolean;
		altKey: boolean;
		shiftKey: boolean;
	}> = {},
) => ({
	code,
	metaKey: false,
	ctrlKey: false,
	altKey: false,
	shiftKey: false,
	...mods,
});

describe("acceleratorFromKeyboardEvent", () => {
	it("maps meta+alt+letter to the canonical token order", () => {
		expect(
			acceleratorFromKeyboardEvent(ev("KeyK", { metaKey: true, altKey: true })),
		).toBe("alt+cmd+k");
	});

	it("maps ctrl+shift+digit", () => {
		expect(
			acceleratorFromKeyboardEvent(
				ev("Digit1", { ctrlKey: true, shiftKey: true }),
			),
		).toBe("ctrl+shift+1");
	});

	it("maps punctuation, F-keys, space, and arrows", () => {
		expect(acceleratorFromKeyboardEvent(ev("Comma", { metaKey: true }))).toBe(
			"cmd+,",
		);
		expect(acceleratorFromKeyboardEvent(ev("F5", { altKey: true }))).toBe(
			"alt+f5",
		);
		expect(acceleratorFromKeyboardEvent(ev("Space", { metaKey: true }))).toBe(
			"cmd+space",
		);
		expect(acceleratorFromKeyboardEvent(ev("ArrowUp", { metaKey: true }))).toBe(
			"cmd+up",
		);
	});

	it("returns null without a primary modifier", () => {
		expect(acceleratorFromKeyboardEvent(ev("KeyK"))).toBeNull();
		expect(
			acceleratorFromKeyboardEvent(ev("KeyK", { shiftKey: true })),
		).toBeNull();
	});

	it("returns null for a modifier-only keydown", () => {
		expect(
			acceleratorFromKeyboardEvent(ev("MetaLeft", { metaKey: true })),
		).toBeNull();
		expect(
			acceleratorFromKeyboardEvent(ev("AltRight", { altKey: true })),
		).toBeNull();
	});

	it("returns null for keys the recorder rejects", () => {
		for (const code of ["Enter", "Escape", "Backspace", "Tab", "Delete"]) {
			expect(
				acceleratorFromKeyboardEvent(ev(code, { metaKey: true })),
			).toBeNull();
		}
	});
});

describe("displayGlyphs", () => {
	it("renders macOS modifier order regardless of stored order", () => {
		expect(displayGlyphs("alt+cmd+k")).toEqual(["⌥", "⌘", "K"]);
		expect(displayGlyphs("cmd+alt+k")).toEqual(["⌥", "⌘", "K"]);
		expect(displayGlyphs("ctrl+shift+f5")).toEqual(["⌃", "⇧", "F5"]);
	});

	it("renders special keys readably", () => {
		expect(displayGlyphs("cmd+space")).toEqual(["⌘", "Space"]);
		expect(displayGlyphs("cmd+up")).toEqual(["⌘", "↑"]);
		expect(displayGlyphs("cmd+,")).toEqual(["⌘", ","]);
	});
});
