import { describe, expect, it } from "vitest";
import { resolveDeepLink } from "@/hooks/use-deep-link-router";

describe("resolveDeepLink", () => {
	it("maps the ideas route to the planner surface", () => {
		expect(resolveDeepLink("ideas")).toEqual({ kind: "planner" });
	});

	it("maps the ingest route to the import sheet", () => {
		expect(resolveDeepLink("ingest")).toEqual({ kind: "ingest" });
	});

	it("maps a project route to that project's detail", () => {
		expect(resolveDeepLink("project/nvme-deep-dive-2026-07-09")).toEqual({
			kind: "project",
			slug: "nvme-deep-dive-2026-07-09",
		});
	});

	it("resolves unknown or slug-less routes to null", () => {
		expect(resolveDeepLink("project/")).toBeNull();
		expect(resolveDeepLink("project/a/b")).toBeNull();
		expect(resolveDeepLink("settings")).toBeNull();
		expect(resolveDeepLink("")).toBeNull();
	});
});
