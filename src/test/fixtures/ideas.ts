import type { Idea } from "@/lib/ipc/ideas";

/** A manually captured backlog idea with all optional columns cleared. */
function manualIdea(
	overrides: Partial<Idea> & Pick<Idea, "id" | "title">,
): Idea {
	return {
		type: "manual",
		kind: "unset",
		status: "backlog",
		rationale: null,
		source: null,
		source_url: null,
		source_title: null,
		evidence_json: null,
		raw_signal_id: null,
		first_seen: "2026-07-08T08:00:00.000Z",
		notes: null,
		promoted_slug: null,
		kind_source: null,
		kind_why: null,
		...overrides,
	};
}

/** Newest-first backlog page for tests; spread-override per scenario. */
export const backlogFixture: Idea[] = [
	manualIdea({
		id: "idea-1",
		title: "NVMe deep dive",
		kind: "long",
		notes: "The controller thermal story",
		first_seen: "2026-07-08T10:00:00.000Z",
	}),
	manualIdea({
		id: "idea-2",
		title: "Why RAID is dead",
		kind: "unset",
		first_seen: "2026-07-08T09:00:00.000Z",
	}),
];
