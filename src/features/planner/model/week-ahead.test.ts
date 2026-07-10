import { describe, expect, it } from "vitest";
import { weekAhead } from "@/features/planner/model/week-ahead";
import { scheduleEntry } from "@/test/fixtures/schedule";

describe("weekAhead", () => {
	const today = "2026-07-09";

	it("keeps entries in the half-open window [today, today+7)", () => {
		const entries = [
			scheduleEntry({ id: 1, date: "2026-07-08" }), // yesterday — out
			scheduleEntry({ id: 2, date: "2026-07-09" }), // today — in (inclusive)
			scheduleEntry({ id: 3, date: "2026-07-15" }), // today+6 — in
			scheduleEntry({ id: 4, date: "2026-07-16" }), // today+7 — out (exclusive)
		];
		expect(weekAhead(entries, today).map((entry) => entry.id)).toEqual([2, 3]);
	});

	it("sorts the window ascending by date regardless of input order", () => {
		const entries = [
			scheduleEntry({ id: 1, date: "2026-07-14" }),
			scheduleEntry({ id: 2, date: "2026-07-10" }),
			scheduleEntry({ id: 3, date: "2026-07-12" }),
		];
		expect(weekAhead(entries, today).map((entry) => entry.date)).toEqual([
			"2026-07-10",
			"2026-07-12",
			"2026-07-14",
		]);
	});

	it("returns nothing when the window is empty", () => {
		const entries = [scheduleEntry({ id: 1, date: "2026-08-01" })];
		expect(weekAhead(entries, today)).toEqual([]);
	});
});
