/**
 * Address-bar normalization. An input is an address when it already parses
 * with an http(s) scheme, or when scheme-less it contains a dot and no
 * spaces. Everything else is null — katto is not a search engine.
 */
export function normalizeAddress(raw: string): string | null {
	const input = raw.trim();
	if (input.length === 0) return null;
	if (/^[a-z][a-z0-9+.-]*:/i.test(input)) {
		try {
			const parsed = new URL(input);
			return parsed.protocol === "http:" || parsed.protocol === "https:"
				? input
				: null;
		} catch {
			return null;
		}
	}
	if (input.includes(" ") || !input.includes(".")) return null;
	try {
		new URL(`https://${input}`);
	} catch {
		return null;
	}
	return `https://${input}`;
}

/** The URL as the address bar shows it: no scheme, no trailing slash. */
export function displayUrl(url: string): string {
	return url.replace(/^https?:\/\//, "").replace(/\/$/, "");
}
