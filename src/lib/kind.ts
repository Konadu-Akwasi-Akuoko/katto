import type { ProjectKind } from "@/lib/ipc/bindings.gen";

/**
 * Display labels for the kind vocabulary, keyed by the stored value. A value
 * outside the set falls through to itself at the call site.
 */
export const KIND_LABELS: Record<string, string> = {
	unset: "Unsorted",
	long: "Long-form",
	short: "Short",
	series: "Series",
};

/** The closed kind vocabulary, in menu order. */
export const PROJECT_KINDS = [
	"unset",
	"long",
	"short",
	"series",
] as const satisfies readonly ProjectKind[];

/** Narrow an arbitrary string to a persistable `ProjectKind`. */
export function isProjectKind(value: string): value is ProjectKind {
	return (PROJECT_KINDS as readonly string[]).includes(value);
}
