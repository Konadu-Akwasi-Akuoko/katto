import type {
	BrowserRect,
	BrowserState,
	TabSnapshot,
} from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { BrowserRect, BrowserState, TabSnapshot };

export const browserKeys = {
	state: ["browser", "state"] as const,
	activeProject: ["browser", "active-project"] as const,
};

/** Open a new tab (activates it). No url → Envato Elements. */
export const browserOpenTab = (url?: string): Promise<number> =>
	unwrap(commands.browserOpenTab(url ?? null));

export const browserCloseTab = (tabId: number): Promise<null> =>
	unwrap(commands.browserCloseTab(tabId));

export const browserSelectTab = (tabId: number): Promise<null> =>
	unwrap(commands.browserSelectTab(tabId));

/** Navigate a tab to a normalized address (see `model/address.ts`). */
export const browserNavigate = (tabId: number, url: string): Promise<null> =>
	unwrap(commands.browserNavigate(tabId, url));

/** Move through a tab's history (-1 back, +1 forward). */
export const browserGo = (tabId: number, delta: number): Promise<null> =>
	unwrap(commands.browserGo(tabId, delta));

export const browserState = (): Promise<BrowserState> =>
	unwrap(commands.browserState());

/** Report the content-host rect so child webviews track the layout. */
export const browserSetBounds = (rect: BrowserRect): Promise<null> =>
	unwrap(commands.browserSetBounds(rect));

export const browserSetVisible = (visible: boolean): Promise<null> =>
	unwrap(commands.browserSetVisible(visible));

/** Override the download filing target (null clears to the derived default). */
export const setActiveAssetProject = (slug: string | null): Promise<null> =>
	unwrap(commands.setActiveAssetProject(slug));

/** The current filing target: override, else most recently touched. */
export const activeAssetProject = (): Promise<string | null> =>
	unwrap(commands.activeAssetProject());

/** File a parked download into the picked project. */
export const fileParkedDownload = (
	downloadId: string,
	slug: string,
): Promise<null> => unwrap(commands.fileParkedDownload(downloadId, slug));

/** Reveal a filed asset in Finder (path is containment-checked backend-side). */
export const revealInProject = (slug: string, relPath: string): Promise<null> =>
	unwrap(commands.revealInProject(slug, relPath));
