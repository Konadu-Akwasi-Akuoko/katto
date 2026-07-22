import { describe, expect, it } from "vitest";

import {
	allPathsIn,
	clipCountLabel,
	defaultProjectSlug,
	formatBytes,
	formatDuration,
	hasEnoughFreeSpace,
	remainingClips,
	selectionTotals,
} from "@/features/ingest/model/select";
import type { CardOffer } from "@/lib/ipc/ingest";

const offer: CardOffer = {
	volume: "/Volumes/SONY",
	kind: "sony",
	total_bytes: 300,
	groups: [
		{
			label: "CLIP",
			clips: [
				{
					path: "CLIP/C0001.MP4",
					name: "C0001.MP4",
					size: 100,
					is_video: true,
					selected: true,
					duration_s: 12.5,
				},
				{
					path: "CLIP/C0001.XML",
					name: "C0001.XML",
					size: 5,
					is_video: false,
					selected: false,
					duration_s: null,
				},
				{
					path: "CLIP/C0002.MP4",
					name: "C0002.MP4",
					size: 200,
					is_video: true,
					selected: true,
					duration_s: null,
				},
			],
		},
	],
};

describe("formatBytes", () => {
	it("renders GB with one decimal", () => {
		expect(formatBytes(22 * 1024 ** 3)).toBe("22.0 GB");
	});
	it("renders MB below a gigabyte", () => {
		expect(formatBytes(500 * 1024 ** 2)).toBe("500 MB");
	});
});

describe("formatDuration", () => {
	it("formats mm:ss", () => {
		expect(formatDuration(72)).toBe("1:12");
	});
	it("shows a dash when unknown", () => {
		expect(formatDuration(null)).toBe("—");
	});
});

describe("selectionTotals", () => {
	it("counts only selected paths and sums their bytes", () => {
		const selected = new Set(["CLIP/C0001.MP4", "CLIP/C0002.MP4"]);
		expect(selectionTotals(offer, selected)).toEqual({ count: 2, bytes: 300 });
	});
});

describe("defaultProjectSlug", () => {
	it("picks the project whose shoot_date is nearest today", () => {
		const projects = [
			{ slug: "far", shoot_date: "2026-07-01" },
			{ slug: "near", shoot_date: "2026-07-21" },
		];
		expect(defaultProjectSlug(projects, "2026-07-22")).toBe("near");
	});
	it("returns null with no projects", () => {
		expect(defaultProjectSlug([], "2026-07-22")).toBeNull();
	});
});

describe("hasEnoughFreeSpace", () => {
	it("passes when free space covers the selection exactly", () => {
		expect(hasEnoughFreeSpace(100, 100)).toBe(true);
	});
	it("fails when the selection exceeds free space", () => {
		expect(hasEnoughFreeSpace(101, 100)).toBe(false);
	});
});

describe("clipCountLabel", () => {
	it("singularizes one clip", () => {
		expect(clipCountLabel(1)).toBe("1 clip");
	});
	it("pluralizes everything else", () => {
		expect(clipCountLabel(0)).toBe("0 clips");
		expect(clipCountLabel(14)).toBe("14 clips");
	});
});

describe("remainingClips", () => {
	const failedEvent = {
		kind: "ingest_failed",
		payload_json: JSON.stringify({
			job_id: "job-1",
			remaining: ["CLIP/C0002.MP4", "CLIP/C0003.MP4"],
		}),
	};

	it("returns the remainder for the matching job", () => {
		const events = [
			{ kind: "ingested", payload_json: "{}" },
			failedEvent,
			{ kind: "ingest_failed", payload_json: '{"job_id":"other"}' },
		];
		expect(remainingClips(events, "job-1")).toEqual([
			"CLIP/C0002.MP4",
			"CLIP/C0003.MP4",
		]);
	});

	it("returns null for unknown jobs or malformed payloads", () => {
		expect(remainingClips([failedEvent], "job-9")).toBeNull();
		expect(
			remainingClips(
				[{ kind: "ingest_failed", payload_json: "not json" }],
				"job-1",
			),
		).toBeNull();
		expect(remainingClips([], "job-1")).toBeNull();
	});
});

describe("allPathsIn", () => {
	it("returns only video clip paths for select-all", () => {
		const group = offer.groups[0];
		if (group === undefined) throw new Error("fixture has no group");
		expect(allPathsIn(group)).toEqual(["CLIP/C0001.MP4", "CLIP/C0002.MP4"]);
	});
});
