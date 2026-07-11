import { describe, expect, it } from "vitest";
import {
	priorityAppearance,
	statusAppearance,
} from "@/features/planner/model/appearance";

describe("statusAppearance", () => {
	it("maps each known status to its label", () => {
		expect(statusAppearance("idea").label).toBe("Idea");
		expect(statusAppearance("shooting").label).toBe("Shooting");
		expect(statusAppearance("editing").label).toBe("Editing");
		expect(statusAppearance("published").label).toBe("Published");
	});

	it("falls back to idea for an unknown status (folders are truth)", () => {
		expect(statusAppearance("archived")).toEqual(statusAppearance("idea"));
	});
});

describe("priorityAppearance", () => {
	it("returns null for none and unknown values (no chrome renders)", () => {
		expect(priorityAppearance("none")).toBeNull();
		expect(priorityAppearance("bogus")).toBeNull();
	});

	it("maps the three real levels to their labels", () => {
		expect(priorityAppearance("low")?.label).toBe("Low");
		expect(priorityAppearance("medium")?.label).toBe("Medium");
		expect(priorityAppearance("high")?.label).toBe("High");
	});
});
