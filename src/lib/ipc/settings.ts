import type { Settings, SettingsPatch } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { Settings, SettingsPatch };

/** Read the full settings object assembled from the key/value table. */
export const getSettings = (): Promise<Settings> =>
	unwrap(commands.getSettings());

/** Apply a partial settings update and return the resulting settings. */
export const setSettings = (patch: SettingsPatch): Promise<Settings> =>
	unwrap(commands.setSettings(patch));

/** TanStack Query key for the settings object. */
export const settingsKeys = { all: ["settings"] as const };

const EMPTY_PATCH: SettingsPatch = {
	studio_root: null,
	default_nle: null,
	idle_reap_minutes: null,
	onboarding_complete: null,
	claude_path: null,
};

/** Patch only the named fields (null in the wire patch means "leave as is"). */
export const patchSettings = (
	patch: Partial<SettingsPatch>,
): Promise<Settings> => setSettings({ ...EMPTY_PATCH, ...patch });
