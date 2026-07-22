import { Channel } from "@tauri-apps/api/core";
import type {
	Edits_Deserialize,
	ExportPreview,
	ExportResult,
	Job,
	JobProgress,
	NleTarget,
} from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

// Feature code never imports the bindings file directly; the editor-domain
// types it needs are re-exported here.
export type {
	EditHistory,
	EditSnapshot,
	Edits_Deserialize,
	Rational,
} from "@/lib/ipc/bindings.gen";
export type { ExportPreview, ExportResult, JobProgress, NleTarget };

/** THE debounced auto-save target — the only interactive-path IPC call. */
export const saveEdits = (
	bundlePath: string,
	edits: Edits_Deserialize,
): Promise<null> => unwrap(commands.saveEdits(bundlePath, edits));

/** Slug, next version, and the sticky NLE default for the export dialog. */
export const previewExport = (bundlePath: string): Promise<ExportPreview> =>
	unwrap(commands.previewExport(bundlePath));

/** Write FCPXML + SRT + VTT at the next version; optionally open/reveal. */
export const exportTimeline = (
	bundlePath: string,
	nleTarget: NleTarget,
	openAfter: boolean,
): Promise<ExportResult> =>
	unwrap(commands.exportTimeline(bundlePath, nleTarget, openAfter));

/** Spawn the kept-only MP4 render job; the backend versions the output path. */
export const renderMp4 = (
	bundlePath: string,
	onProgress: (p: JobProgress) => void,
): Promise<Job> => {
	const channel = new Channel<JobProgress>();
	channel.onmessage = onProgress;
	return unwrap(commands.renderMp4(bundlePath, channel));
};

/** Spawn the thumbnail regeneration job. */
export const generateThumbs = (
	bundlePath: string,
	onProgress: (p: JobProgress) => void,
): Promise<Job> => {
	const channel = new Channel<JobProgress>();
	channel.onmessage = onProgress;
	return unwrap(commands.generateThumbs(bundlePath, channel));
};

/** Native picker for the relocation flow; null when cancelled. */
export const pickRelocationFile = (filename: string): Promise<string | null> =>
	unwrap(commands.pickRelocationFile(filename));

/** Swap the manifest's source path after the same-file check passes. */
export const relocateSource = (
	bundlePath: string,
	newPath: string,
): Promise<null> => unwrap(commands.relocateSource(bundlePath, newPath));

/** `open -a "Final Cut Pro"`; false = FCP missing, file revealed instead. */
export const openInFcp = (path: string): Promise<boolean> =>
	unwrap(commands.openInFcp(path));

/** Reveal an exported artifact in Finder. */
export const revealTimeline = (path: string): Promise<null> =>
	unwrap(commands.revealTimeline(path));
