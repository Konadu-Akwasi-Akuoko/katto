import type {
	CreateThumbnailResult,
	LatestThumb,
	ThumbFormat,
} from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { CreateThumbnailResult, LatestThumb, ThumbFormat };

export const thumbKeys = {
	latest: (slug: string) => ["thumbnails", "latest", slug] as const,
	all: ["thumbnails", "latest-all"] as const,
};

/** Scaffold a guide-lined template PSD and open it (Photoshop or Finder). */
export const createThumbnail = (
	slug: string,
	format: ThumbFormat,
): Promise<CreateThumbnailResult> =>
	unwrap(commands.createThumbnail(slug, format));

/** Newest exported PNG (path + mtime for cache-busting convertFileSrc). */
export const latestThumbnail = (slug: string): Promise<LatestThumb | null> =>
	unwrap(commands.latestThumbnail(slug));

/** Asset URL for a thumb, keyed by mtime so re-exports repaint. */
export const thumbSrc = (
	thumb: Pick<LatestThumb, "path" | "mtime_ms">,
	convert: (path: string) => string,
): string => `${convert(thumb.path)}?v=${thumb.mtime_ms}`;

/** Newest PNG per project (one readdir each) for the projects grid. */
export const listLatestThumbnails = (): Promise<LatestThumb[]> =>
	unwrap(commands.listLatestThumbnails());

/** Watch a project's thumbnails/ dir (replaces the previous watch). */
export const watchThumbnails = (slug: string): Promise<null> =>
	unwrap(commands.watchThumbnails(slug));

export const unwatchThumbnails = (): Promise<null> =>
	unwrap(commands.unwatchThumbnails());
