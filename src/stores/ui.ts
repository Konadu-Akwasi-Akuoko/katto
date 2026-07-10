import { create } from "zustand";

export type Surface = "dashboard" | "planner" | "projects" | "settings";

type UiState = {
	surface: Surface;
	selectedProjectSlug: string | null;
	paletteOpen: boolean;
	setSurface: (surface: Surface) => void;
	openProject: (slug: string) => void;
	setSelectedProjectSlug: (slug: string | null) => void;
	setPaletteOpen: (open: boolean) => void;
	togglePalette: () => void;
};

/**
 * Global UI state: active surface + palette visibility. Read via selectors only.
 * `selectedProjectSlug` is the intra-projects list↔detail selection; switching
 * surfaces clears it, while `openProject` jumps straight to a project's detail
 * (the calendar chip's cross-surface navigation channel).
 */
export const useUiStore = create<UiState>((set) => ({
	surface: "dashboard",
	selectedProjectSlug: null,
	paletteOpen: false,
	setSurface: (surface) => set({ surface, selectedProjectSlug: null }),
	openProject: (slug) =>
		set({ surface: "projects", selectedProjectSlug: slug }),
	setSelectedProjectSlug: (slug) => set({ selectedProjectSlug: slug }),
	setPaletteOpen: (open) => set({ paletteOpen: open }),
	togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
}));
