import { describe, expect, it } from "vitest";
import { displayUrl, normalizeAddress } from "./address";

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

describe("displayUrl", () => {
	it("drops the scheme and trailing slash", () => {
		expect(displayUrl("https://elements.envato.com/")).toBe(
			"elements.envato.com",
		);
		expect(displayUrl("https://a.test/b/c")).toBe("a.test/b/c");
	});
});
