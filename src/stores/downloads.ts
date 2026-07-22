import { create } from "zustand";

export type DownloadRowStatus =
	| "filing"
	| "filed"
	| "failed"
	| "fallback"
	| "needs-project";

export type DownloadRow = {
	id: string;
	filename: string;
	status: DownloadRowStatus;
	project?: string;
	destRel?: string;
};

type DownloadsState = {
	rows: DownloadRow[];
	/** The download the sheet is currently asking a project for, if any. */
	needsProject: { id: string; filename: string } | null;
	upsert: (row: DownloadRow) => void;
	dismiss: (id: string) => void;
	setNeedsProject: (value: { id: string; filename: string } | null) => void;
};

/**
 * Rows for the downloads popover, fed by the download broadcasts via
 * `DownloadsBridge` (mounted in app.tsx so toasts fire on any surface).
 * Ephemeral by design — filed history lives in the events log.
 */
export const useDownloadsStore = create<DownloadsState>((set) => ({
	rows: [],
	needsProject: null,
	upsert: (row) =>
		set((s) => {
			const idx = s.rows.findIndex((r) => r.id === row.id);
			if (idx === -1) return { rows: [row, ...s.rows] };
			const rows = [...s.rows];
			rows[idx] = row;
			return { rows };
		}),
	dismiss: (id) => set((s) => ({ rows: s.rows.filter((r) => r.id !== id) })),
	setNeedsProject: (value) => set({ needsProject: value }),
}));
