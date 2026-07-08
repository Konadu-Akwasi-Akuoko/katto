import { create } from "zustand";

export type Surface = "dashboard" | "planner" | "projects" | "settings";

type UiState = {
	surface: Surface;
	paletteOpen: boolean;
	setSurface: (surface: Surface) => void;
	setPaletteOpen: (open: boolean) => void;
	togglePalette: () => void;
};

/** Global UI state: active surface + palette visibility. Read via selectors only. */
export const useUiStore = create<UiState>((set) => ({
	surface: "dashboard",
	paletteOpen: false,
	setSurface: (surface) => set({ surface }),
	setPaletteOpen: (open) => set({ paletteOpen: open }),
	togglePalette: () => set((s) => ({ paletteOpen: !s.paletteOpen })),
}));
