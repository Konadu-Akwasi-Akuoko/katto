import { describe, expect, it } from "vitest";
import {
	displayUrl,
	normalizeAddress,
	searchUrl,
	toNavigable,
} from "./address";

describe("normalizeAddress", () => {
	it("passes full urls through", () => {
		expect(normalizeAddress("https://elements.envato.com/x")).toBe(
			"https://elements.envato.com/x",
		);
		expect(normalizeAddress("http://a.test/")).toBe("http://a.test/");
	});
	it("prepends https for bare hosts", () => {
		expect(normalizeAddress("elements.envato.com")).toBe(
			"https://elements.envato.com",
		);
		expect(normalizeAddress("example.com/path")).toBe(
			"https://example.com/path",
		);
	});
	it("trims whitespace", () => {
		expect(normalizeAddress("  example.com  ")).toBe("https://example.com");
	});
	it("rejects things that are not addresses", () => {
		expect(normalizeAddress("dust particles")).toBeNull();
		expect(normalizeAddress("")).toBeNull();
		expect(normalizeAddress("file:///etc")).toBeNull();
	});
});

describe("searchUrl", () => {
	it("encodes the query", () => {
		expect(searchUrl("dust particles")).toBe(
			"https://www.google.com/search?q=dust%20particles",
		);
		expect(searchUrl("a & b")).toBe(
			"https://www.google.com/search?q=a%20%26%20b",
		);
	});
});

describe("toNavigable", () => {
	it("passes addresses through", () => {
		expect(toNavigable("example.com")).toBe("https://example.com");
		expect(toNavigable("https://a.test/x")).toBe("https://a.test/x");
	});
	it("searches free text", () => {
		expect(toNavigable("dust particles")).toBe(
			"https://www.google.com/search?q=dust%20particles",
		);
	});
	it("returns null only for empty input", () => {
		expect(toNavigable("")).toBeNull();
		expect(toNavigable("   ")).toBeNull();
	});
	it("searches for a scheme it will not open", () => {
		expect(toNavigable("file:///etc")).toBe(
			"https://www.google.com/search?q=file%3A%2F%2F%2Fetc",
		);
	});
});

describe("displayUrl", () => {
	it("drops the scheme and trailing slash", () => {
		expect(displayUrl("https://elements.envato.com/")).toBe(
			"elements.envato.com",
		);
		expect(displayUrl("https://a.test/b/c")).toBe("a.test/b/c");
	});
});
