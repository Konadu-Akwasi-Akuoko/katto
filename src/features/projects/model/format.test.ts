import { describe, expect, it } from "vitest";
import { formatDate, relativeMtime } from "@/features/projects/model/format";

const now = new Date("2026-07-09T12:00:00.000Z");

describe("relativeMtime", () => {
	it("renders an em dash for an empty subfolder", () => {
		expect(relativeMtime(null, now)).toBe("—");
	});

	it("collapses sub-minute ages to 'just now'", () => {
		expect(relativeMtime("2026-07-09T11:59:30.000Z", now)).toBe("just now");
	});

	it("reports whole minutes within the hour", () => {
		expect(relativeMtime("2026-07-09T11:15:00.000Z", now)).toBe("45m ago");
	});

	it("reports whole hours within the day", () => {
		expect(relativeMtime("2026-07-09T03:00:00.000Z", now)).toBe("9h ago");
	});

	it("falls back to an absolute date past a day", () => {
		expect(relativeMtime("2026-07-01T00:00:00.000Z", now)).toBe("Jul 1");
	});

	it("echoes an unparseable timestamp unchanged", () => {
		expect(relativeMtime("not-a-date", now)).toBe("not-a-date");
	});
});

describe("formatDate", () => {
	it("formats an ISO date as a compact month + day", () => {
		expect(formatDate("2026-07-10")).toBe("Jul 10");
	});

	it("echoes an unparseable date unchanged", () => {
		expect(formatDate("nope")).toBe("nope");
	});
});
