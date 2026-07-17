import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import App from "@/app/app";
import { clearCommands } from "@/features/palette/registry";
import { queryClient } from "@/lib/query-client";
import { useUiStore } from "@/stores/ui";
import { settingsFixture } from "@/test/fixtures/settings";

beforeEach(() => {
	queryClient.clear();
	clearCommands();
	useUiStore.setState({ surface: "dashboard", paletteOpen: false });
	mockIPC((cmd) => {
		switch (cmd) {
			case "get_settings":
				return { ...settingsFixture, onboarding_complete: true };
			case "get_autostart":
				return false;
			case "detect_claude":
				return null;
			case "plugin:event|listen":
				return 1;
			case "plugin:event|unlisten":
				return null;
			case "get_drive_status":
				return { mounted: true, path: null, free_gb: null };
			default:
				throw new Error(`unexpected command ${cmd}`);
		}
	});
});

describe("App navigation", () => {
	it("routes to settings from the sidebar", async () => {
		const user = userEvent.setup();
		render(<App />);

		await user.click(await screen.findByRole("button", { name: "Settings" }));
		expect(
			await screen.findByRole("heading", { name: "Settings", level: 1 }),
		).toBeInTheDocument();
	});

	it("opens the palette on meta+k and navigates via a command", async () => {
		const user = userEvent.setup();
		render(<App />);
		await screen.findByRole("button", { name: "Dashboard" });

		await user.keyboard("{Meta>}k{/Meta}");
		const palette = await screen.findByRole("dialog");
		await user.click(await within(palette).findByText("Open settings"));
		expect(
			await screen.findByRole("heading", { name: "Settings", level: 1 }),
		).toBeInTheDocument();
	});
});
