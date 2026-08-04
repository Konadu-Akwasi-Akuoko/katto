/**
 * Address normalization. An input is an address when it already parses with
 * an http(s) scheme, or when scheme-less it contains a dot and no spaces.
 * Everything else is null; `toNavigable` turns that into a search.
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

const SEARCH_PREFIX = "https://www.google.com/search?q=";

/** Free text as a Google search URL. */
export function searchUrl(query: string): string {
	return `${SEARCH_PREFIX}${encodeURIComponent(query.trim())}`;
}

/**
 * What Enter should load, from either the address bar or the start page:
 * an address when the input parses as one, a Google search otherwise.
 * `null` only for empty input.
 */
export function toNavigable(raw: string): string | null {
	const input = raw.trim();
	if (input.length === 0) return null;
	return normalizeAddress(input) ?? searchUrl(input);
}

/** The URL as the address bar shows it: no scheme, no trailing slash. */
export function displayUrl(url: string): string {
	return url.replace(/^https?:\/\//, "").replace(/\/$/, "");
}
