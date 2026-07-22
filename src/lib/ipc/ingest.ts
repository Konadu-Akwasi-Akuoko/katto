import type {
	CardOffer,
	ClipDto,
	ClipGroupDto,
	Job,
} from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { CardOffer, ClipDto, ClipGroupDto };

export const ingestKeys = {
	all: ["ingest"] as const,
	offer: () => [...ingestKeys.all, "offer"] as const,
};

/** The currently-detected card offer, or null when no card is inserted. */
export const cardOffer = (): Promise<CardOffer | null> =>
	unwrap(commands.cardOffer());

/** Spawn the copy job importing the selected card clips into a project. */
export const startIngest = (
	volume: string,
	projectSlug: string,
	selectedPaths: string[],
): Promise<Job> =>
	unwrap(commands.startIngest(volume, projectSlug, selectedPaths));

/** Eject the card volume (`diskutil eject`). */
export const ejectCard = (volume: string): Promise<null> =>
	unwrap(commands.ejectCard(volume));

/** Manual drag-in import: absolute file paths, same rename+verify pipeline. */
export const importFiles = (
	projectSlug: string,
	paths: string[],
): Promise<Job> => unwrap(commands.importFiles(projectSlug, paths));
