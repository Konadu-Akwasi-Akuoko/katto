import { describe, expect, it } from "vitest";
import { isProjectStatus, PROJECT_STATUSES } from "@/lib/project-status";

describe("project-status", () => {
	it("lists the four v1 statuses in workflow order", () => {
		expect(PROJECT_STATUSES).toEqual([
			"idea",
			"shooting",
			"editing",
			"published",
		]);
	});

	it("rejects a status outside the vocabulary", () => {
		expect(isProjectStatus("editing")).toBe(true);
		expect(isProjectStatus("archived")).toBe(false);
	});
});
