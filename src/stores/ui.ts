import { create } from "zustand";

export type Surface =
	| "dashboard"
	| "planner"
	| "projects"
	| "browser"
	| "settings";

/**
 * A palette command that needs a second step (a picker or a text prompt) opens
 * one of these secondary dialogs instead of resolving inline.
 */
export type PaletteDialog = "promote-idea" | "go-to-project" | "new-project";

type UiState = {
	surface: Surface;
	selectedProjectSlug: string | null;
	peekSlug: string | null;
	paletteOpen: boolean;
	paletteDialog: PaletteDialog | null;
	/** Bundle the read-only cut editor is showing (null = project detail). */
	editorBundlePath: string | null;
	/** Claude dock slide-over visibility + which session tab is active. */
	dockOpen: boolean;
	activeSessionId: string | null;
	/** Slug whose project card announces itself once after a promote. */
	justPromotedSlug: string | null;
	openCutEditor: (bundlePath: string) => void;
	closeCutEditor: () => void;
	toggleDock: () => void;
	openDock: (sessionId?: string) => void;
	closeDock: () => void;
	setActiveSession: (id: string | null) => void;
	setJustPromoted: (slug: string | null) => void;
	setSurface: (surface: Surface) => void;
	openProject: (slug: string) => void;
	setSelectedProjectSlug: (slug: string | null) => void;
	openPeek: (slug: string) => void;
	closePeek: () => void;
	setPaletteOpen: (open: boolean) => void;
	togglePalette: () => void;
	openPaletteDialog: (dialog: PaletteDialog) => void;
	closePaletteDialog: () => void;
};

/**
 * Global UI state: active surface + palette visibility. Read via selectors only.
 * `selectedProjectSlug` is the intra-projects list↔detail selection; switching
 * surfaces clears it, while `openProject` jumps straight to a project's detail
 * (the calendar chip's cross-surface navigation channel). `peekSlug` is the project
 * the shared peek drawer is showing (opened from board/calendar; the drawer is mounted
 * once at the shell). `paletteDialog` is the secondary surface a two-step palette
 * command (promote/go-to/new-project) opens.
 */
export const useUiStore = create<UiState>((set) => ({
	surface: "dashboard",
	selectedProjectSlug: null,
	peekSlug: null,
	paletteOpen: false,
	paletteDialog: null,
	editorBundlePath: null,
	dockOpen: false,
	activeSessionId: null,
	justPromotedSlug: null,
	openCutEditor: (bundlePath) => set({ editorBundlePath: bundlePath }),
	closeCutEditor: () => set({ editorBundlePath: null }),
	toggleDock: () => set((s) => ({ dockOpen: !s.dockOpen })),
	openDock: (sessionId) =>
		set((s) => ({
			dockOpen: true,
			activeSessionId: sessionId ?? s.activeSessionId,
		})),
	closeDock: () => set({ dockOpen: false }),
	setActiveSession: (id) => set({ activeSessionId: id }),
	setJustPromoted: (slug) => set({ justPromotedSlug: slug }),
	setSurface: (surface) =>
		set({ surface, selectedProjectSlug: null, editorBundlePath: null }),
	openProject: (slug) =>
		set({
			surface: "projects",
			selectedProjectSlug: slug,
			editorBundlePath: null,
		}),
	setSelectedProjectSlug: (slug) =>
		set({ selectedProjectSlug: slug, editorBundlePath: null }),
	openPeek: (slug) => set({ peekSlug: slug }),
	closePeek: () => set({ peekSlug: null }),
	setPaletteOpen: (open) => set({ paletteOpen: open }),
	togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
	openPaletteDialog: (dialog) => set({ paletteDialog: dialog }),
	closePaletteDialog: () => set({ paletteDialog: null }),
}));
