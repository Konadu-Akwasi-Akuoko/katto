import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

/** Whether launch-at-login is enabled at the OS level. */
export const getAutostart = (): Promise<boolean> => unwrap(commands.getAutostart());

/** Enable or disable launch-at-login. */
export const setAutostart = (enabled: boolean): Promise<null> =>
	unwrap(commands.setAutostart(enabled));

/** Close the main window; katto stays resident in the tray. */
export const sleepToTray = (): Promise<null> => unwrap(commands.sleepToTray());

/** Quit katto completely. */
export const quitApp = (): Promise<null> => unwrap(commands.quitApp());

/** TanStack Query key for the OS autostart state. */
export const autostartKeys = { all: ["autostart"] as const };
