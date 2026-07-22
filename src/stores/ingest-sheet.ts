import { create } from "zustand";

interface IngestSheetState {
	open: boolean;
	setOpen: (open: boolean) => void;
}

/** Import-sheet visibility. Opened by card detection, the `katto://ingest`
 * deep link, and the palette; a started import keeps it open showing the
 * copy-progress panel until the user closes it. */
export const useIngestSheetStore = create<IngestSheetState>((set) => ({
	open: false,
	setOpen: (open) => set({ open }),
}));
