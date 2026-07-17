import { beforeEach, describe, expect, it } from "vitest";
import { applyTheme, storedTheme } from "@/lib/theme";

beforeEach(() => {
	localStorage.clear();
	document.documentElement.classList.remove("dark");
});

describe("storedTheme", () => {
	it("defaults to dark when nothing is stored", () => {
		expect(storedTheme()).toBe("dark");
	});

	it("defaults to dark on an unknown stored value", () => {
		localStorage.setItem("katto-theme", "solarized");
		expect(storedTheme()).toBe("dark");
	});

	it("round-trips through applyTheme", () => {
		applyTheme("light");
		expect(storedTheme()).toBe("light");
		applyTheme("dark");
		expect(storedTheme()).toBe("dark");
	});
});

describe("applyTheme", () => {
	it("toggles the dark class on the document root", () => {
		applyTheme("dark");
		expect(document.documentElement.classList.contains("dark")).toBe(true);
		applyTheme("light");
		expect(document.documentElement.classList.contains("dark")).toBe(false);
	});
});
