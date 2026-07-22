import { create } from "zustand";

interface IngestSheetState {
	open: boolean;
	setOpen: (open: boolean) => void;
}

/** Import-sheet visibility. Opened by card detection, the `katto://ingest`
 * deep link, and the palette; closed on cancel or a started import. */
export const useIngestSheetStore = create<IngestSheetState>((set) => ({
	open: false,
	setOpen: (open) => set({ open }),
}));
