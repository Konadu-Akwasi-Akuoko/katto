import { describe, expect, it } from "vitest";
import {
	chipsByDate,
	monthGrid,
	weekRow,
} from "@/features/planner/model/calendar";
import { scheduleEntry } from "@/test/fixtures/schedule";

describe("monthGrid", () => {
	// month is 0-indexed, matching Date.getUTCMonth (0 = January, 1 = February).
	it("is always a 6x7 grid regardless of month length", () => {
		const grid = monthGrid(2024, 1);
		expect(grid).toHaveLength(6);
		for (const week of grid) expect(week).toHaveLength(7);
		expect(grid.flat()).toHaveLength(42);
	});

	it("starts each week on Monday and pads the leading days out of month", () => {
		// February 2024 (leap year) begins on a Thursday, so the grid opens on
		// Monday Jan 29 and the first in-month cell is Feb 1 in column 3.
		const grid = monthGrid(2024, 1);
		const first = grid[0];
		if (first === undefined) throw new Error("empty grid");
		expect(first.map((cell) => cell.iso)).toEqual([
			"2024-01-29",
			"2024-01-30",
			"2024-01-31",
			"2024-02-01",
			"2024-02-02",
			"2024-02-03",
			"2024-02-04",
		]);
		expect(first.map((cell) => cell.inMonth)).toEqual([
			false,
			false,
			false,
			true,
			true,
			true,
			true,
		]);
	});

	it("keeps a leap-year February's 29 days all flagged in-month", () => {
		const inMonth = monthGrid(2024, 1)
			.flat()
			.filter((cell) => cell.inMonth);
		expect(inMonth).toHaveLength(29);
		const last = inMonth[inMonth.length - 1];
		expect(last?.iso).toBe("2024-02-29");
		expect(last?.day).toBe(29);
	});

	it("places a Sunday-starting month's 1st in the last column, not the first", () => {
		// September 2024 starts on a Sunday; Monday-first keeps it out of column 0.
		const first = monthGrid(2024, 8)[0];
		if (first === undefined) throw new Error("empty grid");
		expect(first[0]?.iso).toBe("2024-08-26");
		expect(first[0]?.inMonth).toBe(false);
		expect(first[6]?.iso).toBe("2024-09-01");
		expect(first[6]?.inMonth).toBe(true);
	});
});

describe("weekRow", () => {
	it("returns the Monday-through-Sunday week containing the anchor", () => {
		const row = weekRow("2024-02-15");
		expect(row).toHaveLength(7);
		expect(row.map((cell) => cell.iso)).toEqual([
			"2024-02-12",
			"2024-02-13",
			"2024-02-14",
			"2024-02-15",
			"2024-02-16",
			"2024-02-17",
			"2024-02-18",
		]);
		expect(row.every((cell) => cell.inMonth)).toBe(true);
	});

	it("flags cells from a neighbouring month against the anchor's month", () => {
		// Feb 1 2024 is a Thursday: its week reaches back to Monday Jan 29.
		const row = weekRow("2024-02-01");
		expect(row.map((cell) => cell.iso)).toEqual([
			"2024-01-29",
			"2024-01-30",
			"2024-01-31",
			"2024-02-01",
			"2024-02-02",
			"2024-02-03",
			"2024-02-04",
		]);
		expect(row.map((cell) => cell.inMonth)).toEqual([
			false,
			false,
			false,
			true,
			true,
			true,
			true,
		]);
	});
});

describe("chipsByDate", () => {
	it("buckets entries by ISO date, preserving input order within a day", () => {
		const a = scheduleEntry({ id: 1, date: "2026-07-10", kind: "shoot" });
		const b = scheduleEntry({ id: 2, date: "2026-07-10", kind: "publish" });
		const c = scheduleEntry({ id: 3, date: "2026-07-12", kind: "shoot" });
		const map = chipsByDate([a, b, c]);
		expect(map.size).toBe(2);
		expect(map.get("2026-07-10")).toEqual([a, b]);
		expect(map.get("2026-07-12")).toEqual([c]);
		expect(map.get("2026-07-11")).toBeUndefined();
	});

	it("returns an empty map for no entries", () => {
		expect(chipsByDate([]).size).toBe(0);
	});
});
