export type Lean = "hold" | "lean" | "strong";

/**
 * Read the curation run's lean out of an idea's `evidence_json`. The JSON is
 * written by an external tool, so anything malformed or unknown degrades to
 * `null` (no notch) rather than throwing.
 */
export function parseLean(evidenceJson: string | null): Lean | null {
	if (evidenceJson === null) return null;
	let parsed: unknown;
	try {
		parsed = JSON.parse(evidenceJson);
	} catch {
		return null;
	}
	if (typeof parsed !== "object" || parsed === null) return null;
	const lean = (parsed as Record<string, unknown>).lean;
	return lean === "hold" || lean === "lean" || lean === "strong" ? lean : null;
}

/** True when `url` is a well-formed http(s) URL — the only thing the browser opens. */
export function isHttpUrl(url: string): boolean {
	try {
		const { protocol } = new URL(url);
		return protocol === "http:" || protocol === "https:";
	} catch {
		return false;
	}
}

/** The hostname of a source link, or `null` when absent or unparseable. */
export function sourceDomain(url: string | null): string | null {
	if (url === null) return null;
	try {
		return new URL(url).hostname || null;
	} catch {
		return null;
	}
}
