import { describe, expect, it } from "vitest";
import { parseLean, sourceDomain } from "./lean";

describe("parseLean", () => {
	it("returns null for absent evidence", () => {
		expect(parseLean(null)).toBeNull();
	});

	it("returns null for garbage json", () => {
		expect(parseLean("not json {")).toBeNull();
	});

	it("returns null for json without a lean field", () => {
		expect(parseLean('{"signals": 3}')).toBeNull();
	});

	it("parses the three lean values", () => {
		expect(parseLean('{"lean":"hold"}')).toBe("hold");
		expect(parseLean('{"lean":"lean"}')).toBe("lean");
		expect(parseLean('{"lean":"strong"}')).toBe("strong");
	});

	it("returns null for unknown lean strings", () => {
		expect(parseLean('{"lean":"very-strong"}')).toBeNull();
		expect(parseLean('{"lean":7}')).toBeNull();
	});
});

describe("sourceDomain", () => {
	it("returns the hostname of a valid url", () => {
		expect(sourceDomain("https://news.ycombinator.com/item?id=1")).toBe(
			"news.ycombinator.com",
		);
	});

	it("returns null for absent or invalid urls", () => {
		expect(sourceDomain(null)).toBeNull();
		expect(sourceDomain("not a url")).toBeNull();
	});
});
