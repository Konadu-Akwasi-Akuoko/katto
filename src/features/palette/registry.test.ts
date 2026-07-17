import { afterEach, describe, expect, it, vi } from "vitest";
import {
	clearCommands,
	listCommands,
	registerCommand,
} from "@/features/palette/registry";

const cmd = (id: string, title = id) => ({
	id,
	title,
	keywords: [],
	group: "Test",
	run: vi.fn(),
});

afterEach(() => {
	clearCommands();
});

describe("palette registry", () => {
	it("lists commands in registration order", () => {
		registerCommand(cmd("a"));
		registerCommand(cmd("b"));
		expect(listCommands().map((c) => c.id)).toEqual(["a", "b"]);
	});

	it("replaces by id without changing position", () => {
		registerCommand(cmd("a", "first"));
		registerCommand(cmd("b"));
		registerCommand(cmd("a", "renamed"));
		expect(listCommands().map((c) => c.title)).toEqual(["renamed", "b"]);
	});

	it("clears to empty", () => {
		registerCommand(cmd("a"));
		clearCommands();
		expect(listCommands()).toEqual([]);
	});
});
