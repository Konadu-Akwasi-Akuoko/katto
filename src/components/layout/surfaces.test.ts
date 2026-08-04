import { describe, expect, it } from "vitest";
import { STUDIO_NAV, SURFACE_NAV } from "@/components/layout/surfaces";

describe("surface nav", () => {
	it("covers every surface exactly once", () => {
		const surfaces = SURFACE_NAV.map((s) => s.surface);
		expect(surfaces).toEqual([
			"dashboard",
			"planner",
			"projects",
			"browser",
			"settings",
		]);
	});

	// a stringly-typed filter: rename the surface and the sidebar silently grows
	// a second Settings entry, so pin the split rather than trusting the literal
	it("splits settings out of the studio group without losing it", () => {
		expect(STUDIO_NAV.map((s) => s.surface)).not.toContain("settings");
		expect(STUDIO_NAV).toHaveLength(SURFACE_NAV.length - 1);
	});
});
