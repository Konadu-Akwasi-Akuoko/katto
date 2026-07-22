/**
 * Read the daily time out of a `daily@HH:MM;catchup=<N>h` spec. Null on
 * anything malformed — the UI shows an empty field rather than guessing.
 */
export function parseSpecTime(
	spec: string,
): { hour: number; minute: number } | null {
	const match = /^daily@(\d{2}):(\d{2});catchup=\d+h$/.exec(spec);
	if (match === null) return null;
	const hour = Number(match[1]);
	const minute = Number(match[2]);
	if (hour > 23 || minute > 59) return null;
	return { hour, minute };
}
