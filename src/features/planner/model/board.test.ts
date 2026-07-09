import { describe, expect, it } from "vitest";
import {
	BOARD_COLUMNS,
	groupByStatus,
	isBoardColumn,
} from "@/features/planner/model/board";
import { project } from "@/test/fixtures/projects";

describe("board model", () => {
	it("declares the four v1 columns in workflow order", () => {
		expect(BOARD_COLUMNS).toEqual(["idea", "shooting", "editing", "published"]);
	});

	it("groups projects into every column, keyed even when empty", () => {
		const groups = groupByStatus([
			project({ slug: "a-2026-07-01", title: "A", status: "idea" }),
			project({ slug: "b-2026-07-01", title: "B", status: "published" }),
		]);
		expect(Object.keys(groups)).toEqual(BOARD_COLUMNS);
		expect(groups.idea.map((p) => p.slug)).toEqual(["a-2026-07-01"]);
		expect(groups.shooting).toEqual([]);
		expect(groups.editing).toEqual([]);
		expect(groups.published.map((p) => p.slug)).toEqual(["b-2026-07-01"]);
	});

	it("falls an unknown status back to idea without logging", () => {
		const groups = groupByStatus([
			project({ slug: "stale-2026-07-01", title: "Stale", status: "archived" }),
		]);
		expect(groups.idea.map((p) => p.slug)).toEqual(["stale-2026-07-01"]);
	});

	it("orders each column most-recently-touched first, untouched last", () => {
		const groups = groupByStatus([
			project({
				slug: "old-2026-07-01",
				title: "Old",
				status: "idea",
				last_touched_at: "2026-07-01T00:00:00.000Z",
			}),
			project({
				slug: "never-2026-07-01",
				title: "Never",
				status: "idea",
				last_touched_at: null,
			}),
			project({
				slug: "new-2026-07-01",
				title: "New",
				status: "idea",
				last_touched_at: "2026-07-09T00:00:00.000Z",
			}),
		]);
		expect(groups.idea.map((p) => p.slug)).toEqual([
			"new-2026-07-01",
			"old-2026-07-01",
			"never-2026-07-01",
		]);
	});

	it("recognises only the four columns as board columns", () => {
		expect(isBoardColumn("editing")).toBe(true);
		expect(isBoardColumn("archived")).toBe(false);
	});
});
