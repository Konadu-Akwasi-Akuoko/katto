import { QueryClient } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { registerAppCommands } from "@/app/commands";
import type { PaletteCommand } from "@/features/palette/registry";
import { clearCommands, listCommands } from "@/features/palette/registry";
import { useUiStore } from "@/stores/ui";
import { settingsFixture } from "@/test/fixtures/settings";

function register(): void {
	registerAppCommands(new QueryClient());
}

function byId(id: string): PaletteCommand {
	const command = listCommands().find((c) => c.id === id);
	if (!command) throw new Error(`command ${id} not registered`);
	return command;
}

beforeEach(() => {
	useUiStore.setState({
		surface: "dashboard",
		selectedProjectSlug: null,
		paletteDialog: null,
	});
});

afterEach(() => {
	clearCommands();
	clearMocks();
});

describe("planner palette commands", () => {
	it("registers the five new commands with their titles and groups", () => {
		register();
		expect(byId("plan.new-idea")).toMatchObject({
			title: "New idea",
			group: "Plan",
		});
		expect(byId("plan.promote-idea")).toMatchObject({
			title: "Promote idea…",
			group: "Plan",
		});
		expect(byId("project.new")).toMatchObject({
			title: "New project",
			group: "Project",
		});
		expect(byId("project.goto")).toMatchObject({
			title: "Go to project…",
			group: "Project",
		});
		expect(byId("app.open-studio-root")).toMatchObject({
			title: "Open studio root",
			group: "App",
		});
	});

	it("new idea navigates to the planner surface", () => {
		register();
		byId("plan.new-idea").run();
		expect(useUiStore.getState().surface).toBe("planner");
	});

	it("promote idea opens the promote-idea picker", () => {
		register();
		byId("plan.promote-idea").run();
		expect(useUiStore.getState().paletteDialog).toBe("promote-idea");
	});

	it("new project opens the new-project dialog", () => {
		register();
		byId("project.new").run();
		expect(useUiStore.getState().paletteDialog).toBe("new-project");
	});

	it("go to project opens the go-to-project picker", () => {
		register();
		byId("project.goto").run();
		expect(useUiStore.getState().paletteDialog).toBe("go-to-project");
	});

	it("open studio root reveals the configured root in Finder", async () => {
		const revealed: string[][] = [];
		mockIPC((cmd, payload) => {
			if (cmd === "get_settings") {
				return { ...settingsFixture, studio_root: "/Volumes/studio" };
			}
			if (cmd === "plugin:opener|reveal_item_in_dir") {
				revealed.push((payload as { paths: string[] }).paths);
				return null;
			}
			throw new Error(`unexpected command ${cmd}`);
		});
		register();

		await byId("app.open-studio-root").run();

		expect(revealed).toEqual([["/Volumes/studio"]]);
	});

	it("open studio root throws when no root is configured", async () => {
		mockIPC((cmd) => {
			if (cmd === "get_settings") return settingsFixture;
			throw new Error(`unexpected command ${cmd}`);
		});
		register();

		await expect(byId("app.open-studio-root").run()).rejects.toThrow(
			"No studio root configured.",
		);
	});
});
