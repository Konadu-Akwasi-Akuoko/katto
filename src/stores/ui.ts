import { create } from "zustand";

export type Surface = "dashboard" | "planner" | "projects" | "settings";

/**
 * A palette command that needs a second step (a picker or a text prompt) opens
 * one of these secondary dialogs instead of resolving inline.
 */
export type PaletteDialog = "promote-idea" | "go-to-project" | "new-project";

type UiState = {
	surface: Surface;
	selectedProjectSlug: string | null;
	paletteOpen: boolean;
	paletteDialog: PaletteDialog | null;
	setSurface: (surface: Surface) => void;
	openProject: (slug: string) => void;
	setSelectedProjectSlug: (slug: string | null) => void;
	setPaletteOpen: (open: boolean) => void;
	togglePalette: () => void;
	openPaletteDialog: (dialog: PaletteDialog) => void;
	closePaletteDialog: () => void;
};

/**
 * Global UI state: active surface + palette visibility. Read via selectors only.
 * `selectedProjectSlug` is the intra-projects list↔detail selection; switching
 * surfaces clears it, while `openProject` jumps straight to a project's detail
 * (the calendar chip's cross-surface navigation channel). `paletteDialog` is the
 * secondary surface a two-step palette command (promote/go-to/new-project) opens.
 */
export const useUiStore = create<UiState>((set) => ({
	surface: "dashboard",
	selectedProjectSlug: null,
	paletteOpen: false,
	paletteDialog: null,
	setSurface: (surface) => set({ surface, selectedProjectSlug: null }),
	openProject: (slug) =>
		set({ surface: "projects", selectedProjectSlug: slug }),
	setSelectedProjectSlug: (slug) => set({ selectedProjectSlug: slug }),
	setPaletteOpen: (open) => set({ paletteOpen: open }),
	togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
	openPaletteDialog: (dialog) => set({ paletteDialog: dialog }),
	closePaletteDialog: () => set({ paletteDialog: null }),
}));
