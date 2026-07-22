import type { CardOffer, ClipDto, ClipGroupDto } from "@/lib/ipc/ingest";

/** Human-readable byte size: GB with one decimal at/above 1 GiB, else whole MB. */
export function formatBytes(n: number): string {
	const gib = 1024 ** 3;
	if (n >= gib) return `${(n / gib).toFixed(1)} GB`;
	return `${Math.round(n / 1024 ** 2)} MB`;
}

/** Duration as `m:ss`, or an em dash when unknown. */
export function formatDuration(s: number | null): string {
	if (s === null) return "—";
	const total = Math.round(s);
	const mins = Math.floor(total / 60);
	const secs = total % 60;
	return `${mins}:${secs.toString().padStart(2, "0")}`;
}

/** Every video clip path in a group (used for the per-group select-all). */
export function allPathsIn(group: ClipGroupDto): string[] {
	return group.clips.filter((c) => c.is_video).map((c) => c.path);
}

/** The selected clips across all groups. */
export function selectedClips(
	offer: CardOffer,
	selected: ReadonlySet<string>,
): ClipDto[] {
	return offer.groups.flatMap((g) =>
		g.clips.filter((c) => selected.has(c.path)),
	);
}

/** Count and total bytes of the current selection. */
export function selectionTotals(
	offer: CardOffer,
	selected: ReadonlySet<string>,
): { count: number; bytes: number } {
	const clips = selectedClips(offer, selected);
	return {
		count: clips.length,
		bytes: clips.reduce((sum, c) => sum + c.size, 0),
	};
}

/** True when free space covers the selection. */
export function hasEnoughFreeSpace(bytes: number, freeBytes: number): boolean {
	return freeBytes >= bytes;
}

interface ProjectLike {
	slug: string;
	shoot_date: string | null;
}

/** The project whose `shoot_date` is nearest `today` (ISO `YYYY-MM-DD`), or null. */
export function defaultProjectSlug(
	projects: readonly ProjectLike[],
	today: string,
): string | null {
	const dated = projects.filter(
		(p): p is ProjectLike & { shoot_date: string } => !!p.shoot_date,
	);
	if (dated.length === 0) return projects[0]?.slug ?? null;
	const todayMs = Date.parse(today);
	let best = dated[0]!;
	let bestDelta = Math.abs(Date.parse(best.shoot_date) - todayMs);
	for (const p of dated.slice(1)) {
		const delta = Math.abs(Date.parse(p.shoot_date) - todayMs);
		if (delta < bestDelta) {
			best = p;
			bestDelta = delta;
		}
	}
	return best.slug;
}
