import { commands } from "@/lib/ipc/bindings.gen";
import type { Settings, SettingsPatch } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { Settings, SettingsPatch };

/** Read the full settings object assembled from the key/value table. */
export const getSettings = (): Promise<Settings> => unwrap(commands.getSettings());

/** Apply a partial settings update and return the resulting settings. */
export const setSettings = (patch: SettingsPatch): Promise<Settings> =>
	unwrap(commands.setSettings(patch));
