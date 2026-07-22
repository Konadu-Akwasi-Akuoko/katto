import { isTauri } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { onDeepLinkOpened } from "@/lib/ipc/broadcast";
import { useIngestSheetStore } from "@/stores/ingest-sheet";
import { useUiStore } from "@/stores/ui";

/** A resolved navigation intent from a `katto://` deep-link route string. */
export type DeepLinkTarget =
	| { kind: "planner" }
	| { kind: "ingest" }
	| { kind: "project"; slug: string };

/**
 * Map a wire route (`"ideas"`, `"ingest"`, or `"project/<slug>"`, the backend's
 * `Route::as_wire`) to a navigation intent. Unknown or slug-less routes resolve
 * to `null` — nothing navigates on junk.
 */
export function resolveDeepLink(route: string): DeepLinkTarget | null {
	if (route === "ideas") return { kind: "planner" };
	if (route === "ingest") return { kind: "ingest" };
	if (route.startsWith("project/")) {
		const slug = route.slice("project/".length);
		if (slug.length > 0 && !slug.includes("/")) {
			return { kind: "project", slug };
		}
	}
	return null;
}

/**
 * Navigate the main window when a `katto://` deep link is opened (notification
 * click or OS open). No-op outside Tauri (tests, plain browser), mirroring the
 * broadcast-invalidation guard.
 */
export function useDeepLinkRouter(): void {
	const setSurface = useUiStore((s) => s.setSurface);
	const openProject = useUiStore((s) => s.openProject);

	useEffect(() => {
		if (!isTauri()) return;
		const subscription = onDeepLinkOpened((payload) => {
			const target = resolveDeepLink(payload.route);
			if (target === null) return;
			if (target.kind === "planner") setSurface("planner");
			else if (target.kind === "ingest")
				useIngestSheetStore.getState().setOpen(true);
			else openProject(target.slug);
		});
		return () => {
			void subscription.then((unlisten) => unlisten());
		};
	}, [setSurface, openProject]);
}
