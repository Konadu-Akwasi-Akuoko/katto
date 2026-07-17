import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * The current Tauri window's label, defaulting to `main` when the Tauri context
 * is absent (tests, plain browser).
 */
export function windowLabel(): string {
	try {
		return getCurrentWindow().label;
	} catch {
		return "main";
	}
}
