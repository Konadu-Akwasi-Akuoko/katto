import { describe, expect, it } from "vitest";
import type { RootCheck } from "@/lib/ipc/onboarding";
import { canContinue, rootWarnings } from "@/lib/root-warnings";

const good: RootCheck = {
	path: "/Volumes/Studio",
	writable: true,
	on_boot_volume: false,
	free_gb: 900,
	low_free_space: false,
};

describe("rootWarnings", () => {
	it("is silent for a writable external root with space", () => {
		expect(rootWarnings(good)).toEqual([]);
	});

	it("warns for boot volume and low space independently", () => {
		const warnings = rootWarnings({
			...good,
			on_boot_volume: true,
			free_gb: 42,
			low_free_space: true,
		});
		expect(warnings).toHaveLength(2);
		expect(warnings[0]).toMatch(/internal drive/i);
		expect(warnings[1]).toMatch(/42 GB/);
	});

	it("warns about unwritable roots", () => {
		expect(rootWarnings({ ...good, writable: false })[0]).toMatch(
			/can't write/i,
		);
	});
});

describe("canContinue", () => {
	it("requires a picked, writable root", () => {
		expect(canContinue(null)).toBe(false);
		expect(canContinue({ ...good, writable: false })).toBe(false);
		expect(canContinue(good)).toBe(true);
	});

	it("allows advisory-warned roots through", () => {
		expect(
			canContinue({ ...good, on_boot_volume: true, low_free_space: true }),
		).toBe(true);
	});
});
