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

/** Absolute path of the newest exported PNG, for convertFileSrc. */
export const latestThumbnail = (slug: string): Promise<string | null> =>
	unwrap(commands.latestThumbnail(slug));

/** Newest PNG per project (one readdir each) for the projects grid. */
export const listLatestThumbnails = (): Promise<LatestThumb[]> =>
	unwrap(commands.listLatestThumbnails());

/** Watch a project's thumbnails/ dir (replaces the previous watch). */
export const watchThumbnails = (slug: string): Promise<null> =>
	unwrap(commands.watchThumbnails(slug));

export const unwatchThumbnails = (): Promise<null> =>
	unwrap(commands.unwatchThumbnails());
