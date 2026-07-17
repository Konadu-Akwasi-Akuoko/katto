/**
 * ISO date (`2026-07-10`) or timestamp → compact `Jul 10`, UTC-anchored to
 * dodge local-timezone drift. An unparseable value echoes back unchanged.
 */
export function formatShortDate(iso: string): string {
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return iso;
	return date.toLocaleDateString("en-US", {
		month: "short",
		day: "numeric",
		timeZone: "UTC",
	});
}
