import { describe, expect, it } from "vitest";
import {
	ALL_PHASES,
	addDaysIso,
	addMonthsIso,
	applyCalendarFilters,
	type CalendarFilters,
	chipsByDate,
	markersByDate,
	monthGrid,
	periodLabel,
	weekRow,
} from "@/features/planner/model/calendar";
import type { CalendarMarker } from "@/lib/ipc/calendar";
import { scheduleEntry } from "@/test/fixtures/schedule";

const MARKERS: CalendarMarker[] = [
	{
		kind: "shoot",
		project_slug: "a",
		title: "A",
		date: "2026-07-10",
		note: null,
	},
	{
		kind: "publish",
		project_slug: "b",
		title: "B",
		date: "2026-07-11",
		note: null,
	},
	{ kind: "backlog", idea_id: "i1", title: "Idea", date: "2026-07-12" },
	{
		kind: "phase",
		project_slug: "a",
		title: "A",
		date: "2026-07-13",
		to: "editing",
	},
];

const ALL_ON: CalendarFilters["categories"] = {
	shoot: true,
	publish: true,
	backlog: true,
	phase: true,
};

describe("applyCalendarFilters", () => {
	it("hides categories that are toggled off", () => {
		const out = applyCalendarFilters(MARKERS, {
			categories: { ...ALL_ON, publish: false },
			phases: ALL_PHASES,
		});
		expect(out.map((m) => m.kind)).toEqual(["shoot", "backlog", "phase"]);
	});

	it("keeps phase moves only for the selected destination phases", () => {
		const out = applyCalendarFilters(MARKERS, {
			categories: ALL_ON,
			phases: ["shooting"],
		});
		expect(out.some((m) => m.kind === "phase")).toBe(false);
	});
});

describe("markersByDate", () => {
	it("groups markers by their ISO day", () => {
		const map = markersByDate(MARKERS);
		expect(map.get("2026-07-10")).toHaveLength(1);
		expect(map.get("2026-07-99")).toBeUndefined();
	});
});

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

describe("addMonthsIso", () => {
	it("snaps to the first of the resulting month", () => {
		expect(addMonthsIso("2024-02-15", 1)).toBe("2024-03-01");
	});

	it("rolls the year over stepping past December", () => {
		expect(addMonthsIso("2024-12-10", 1)).toBe("2025-01-01");
	});

	it("rolls the year back stepping before January", () => {
		expect(addMonthsIso("2024-01-31", -1)).toBe("2023-12-01");
	});
});

describe("addDaysIso", () => {
	it("keeps the day within a month", () => {
		expect(addDaysIso("2024-02-10", 3)).toBe("2024-02-13");
	});

	it("crosses into the next month, honouring a leap day", () => {
		expect(addDaysIso("2024-02-28", 1)).toBe("2024-02-29");
		expect(addDaysIso("2024-02-29", 1)).toBe("2024-03-01");
	});

	it("crosses the year boundary in both directions", () => {
		expect(addDaysIso("2024-12-31", 1)).toBe("2025-01-01");
		expect(addDaysIso("2024-01-01", -1)).toBe("2023-12-31");
	});

	it("steps a full week for the week-mode paging stride", () => {
		expect(addDaysIso("2024-02-15", 7)).toBe("2024-02-22");
	});
});

describe("periodLabel", () => {
	it("names the month and year in month mode", () => {
		expect(periodLabel("2024-02-15", "month")).toBe("February 2024");
	});

	it("spans the Monday-through-Sunday week in week mode", () => {
		expect(periodLabel("2024-02-15", "week")).toBe("Feb 12 – Feb 18");
	});

	it("spans a week straddling a month boundary in week mode", () => {
		expect(periodLabel("2024-02-01", "week")).toBe("Jan 29 – Feb 4");
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
