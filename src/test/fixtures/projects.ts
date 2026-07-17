import type {
	FolderFreshness,
	Project,
	ProjectDetail,
} from "@/lib/ipc/projects";

/** A project row with sensible defaults; spread-override per scenario. */
export function project(
	overrides: Partial<Project> & Pick<Project, "slug" | "title">,
): Project {
	return {
		root_path: `/studio/Projects/${overrides.slug}`,
		status: "idea",
		target_nle: "resolve",
		priority: "none",
		shoot_date: null,
		publish_date: null,
		created_at: "2026-07-01T00:00:00.000Z",
		last_touched_at: null,
		...overrides,
	};
}

/** Per-subfolder freshness with a couple of populated folders and empties. */
export const freshnessFixture: FolderFreshness[] = [
	{
		subfolder: "footage",
		file_count: 12,
		latest_mtime: "2026-07-08T10:00:00.000Z",
	},
	{
		subfolder: "audio",
		file_count: 3,
		latest_mtime: "2026-07-08T09:00:00.000Z",
	},
	{ subfolder: "thumbnails", file_count: 0, latest_mtime: null },
	{ subfolder: "exports", file_count: 0, latest_mtime: null },
];

/** A project-detail payload; defaults to a valid manifest + populated grid. */
export function projectDetail(
	proj: Project,
	overrides: Partial<Omit<ProjectDetail, "project">> = {},
): ProjectDetail {
	return {
		project: proj,
		manifest_error: null,
		freshness: freshnessFixture,
		...overrides,
	};
}

/** One project per board column, most-recently-touched staggered for ordering. */
export const boardFixture: Project[] = [
	project({
		slug: "nvme-deep-dive-2026-07-08",
		title: "NVMe deep dive",
		status: "editing",
		shoot_date: "2026-07-10",
		last_touched_at: "2026-07-08T10:00:00.000Z",
	}),
	project({
		slug: "why-raid-is-dead-2026-07-07",
		title: "Why RAID is dead",
		status: "idea",
		last_touched_at: "2026-07-07T09:00:00.000Z",
	}),
	project({
		slug: "thermal-throttling-2026-07-06",
		title: "Thermal throttling",
		status: "shooting",
		shoot_date: "2026-07-12",
		last_touched_at: "2026-07-06T08:00:00.000Z",
	}),
	project({
		slug: "raid-rebuild-2026-07-05",
		title: "RAID rebuild diary",
		status: "published",
		publish_date: "2026-07-04",
		last_touched_at: "2026-07-05T07:00:00.000Z",
	}),
];
