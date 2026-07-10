import type { QueryClient } from "@tanstack/react-query";
import { registerCommand } from "@/features/palette/registry";
import { detectClaude } from "@/lib/ipc/onboarding";
import { settingsKeys } from "@/lib/ipc/settings";
import { quitApp, sleepToTray } from "@/lib/ipc/shell";
import { useUiStore } from "@/stores/ui";

/** Register the Phase-1 palette commands. Idempotent — the registry replaces by id. */
export function registerAppCommands(queryClient: QueryClient): void {
	registerCommand({
		id: "nav.dashboard",
		title: "Open dashboard",
		keywords: ["home", "activity", "jobs"],
		group: "Navigate",
		run: () => useUiStore.getState().setSurface("dashboard"),
	});
	registerCommand({
		id: "nav.projects",
		title: "Open projects",
		keywords: ["project", "folders", "detail", "freshness"],
		group: "Navigate",
		run: () => useUiStore.getState().setSurface("projects"),
	});
	registerCommand({
		id: "nav.settings",
		title: "Open settings",
		keywords: ["preferences", "keys", "root", "autostart"],
		group: "Navigate",
		run: () => useUiStore.getState().setSurface("settings"),
	});
	registerCommand({
		id: "app.sleep",
		title: "Sleep to tray",
		keywords: ["close", "hide", "minimize"],
		group: "App",
		run: async () => {
			await sleepToTray();
		},
	});
	registerCommand({
		id: "app.quit",
		title: "Quit katto",
		keywords: ["exit"],
		group: "App",
		run: async () => {
			await quitApp();
		},
	});
	registerCommand({
		id: "ai.redetect-claude",
		title: "Re-run claude detection",
		keywords: ["claude", "path", "cli", "detect"],
		group: "AI",
		run: async () => {
			await detectClaude();
			await queryClient.invalidateQueries({ queryKey: settingsKeys.all });
		},
	});
}
