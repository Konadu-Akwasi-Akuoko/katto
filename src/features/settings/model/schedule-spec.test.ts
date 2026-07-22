import { describe, expect, it } from "vitest";
import { parseSpecTime } from "./schedule-spec";

describe("parseSpecTime", () => {
	it("reads the daily time out of a spec", () => {
		expect(parseSpecTime("daily@02:30;catchup=20h")).toEqual({
			hour: 2,
			minute: 30,
		});
		expect(parseSpecTime("daily@00:00;catchup=20h")).toEqual({
			hour: 0,
			minute: 0,
		});
	});

	it("is null on malformed specs", () => {
		expect(parseSpecTime("weekly@02:30;catchup=20h")).toBeNull();
		expect(parseSpecTime("daily@25:00;catchup=20h")).toBeNull();
		expect(parseSpecTime("garbage")).toBeNull();
		expect(parseSpecTime("")).toBeNull();
	});
});
