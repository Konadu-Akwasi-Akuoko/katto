import { describe, expect, it } from "vitest";
import { formatShortDate } from "@/lib/date";

describe("formatShortDate", () => {
	it("formats an ISO date as a compact month + day", () => {
		expect(formatShortDate("2026-07-10")).toBe("Jul 10");
	});

	it("anchors to UTC so a late-day date does not drift to the previous day", () => {
		expect(formatShortDate("2026-01-01")).toBe("Jan 1");
	});

	it("accepts a full ISO timestamp", () => {
		expect(formatShortDate("2026-07-10T23:30:00.000Z")).toBe("Jul 10");
	});

	it("echoes an unparseable value unchanged", () => {
		expect(formatShortDate("nope")).toBe("nope");
	});
});
