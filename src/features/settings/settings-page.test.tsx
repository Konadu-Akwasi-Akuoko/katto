import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { SettingsPage } from "@/features/settings/settings-page";
import { settingsFixture } from "@/test/fixtures/settings";

function mockSettingsIpc() {
	const calls: [string, unknown][] = [];
	let autostart = false;
	let elevenlabsPresent = false;
	let claudePath: string | null = null;

	mockIPC((cmd, args) => {
		calls.push([cmd, args]);
		switch (cmd) {
			case "get_settings":
				return {
					...settingsFixture,
					studio_root: "/Volumes/Studio",
					onboarding_complete: true,
					claude_path: claudePath,
					keys_present: { elevenlabs: elevenlabsPresent, anthropic: false },
				};
			case "get_autostart":
				return autostart;
			case "set_autostart":
				autostart = (args as { enabled: boolean }).enabled;
				return null;
			case "store_key":
				elevenlabsPresent = true;
				return null;
			case "detect_claude":
				claudePath = "/opt/homebrew/bin/claude";
				return claudePath;
			case "set_settings":
				return { ...settingsFixture, onboarding_complete: true };
			default:
				throw new Error(`unexpected command ${cmd}`);
		}
	});
	return calls;
}

function renderPage() {
	const qc = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	});
	return render(
		<QueryClientProvider client={qc}>
			<SettingsPage />
		</QueryClientProvider>,
	);
}

describe("SettingsPage", () => {
	it("toggles launch at login through the autostart command", async () => {
		const calls = mockSettingsIpc();
		const user = userEvent.setup();
		renderPage();

		const toggle = await screen.findByRole("switch", { name: /launch at login/i });
		expect(toggle).not.toBeChecked();
		await user.click(toggle);

		await waitFor(() =>
			expect(calls).toContainEqual(["set_autostart", { enabled: true }]),
		);
		await waitFor(() =>
			expect(screen.getByRole("switch", { name: /launch at login/i })).toBeChecked(),
		);
	});

	it("stores a key write-only and reflects presence", async () => {
		mockSettingsIpc();
		const user = userEvent.setup();
		renderPage();

		const input = await screen.findByLabelText("ElevenLabs API key");
		await user.type(input, "xi-secret");
		await user.click(screen.getByRole("button", { name: "Save ElevenLabs key" }));

		await waitFor(() => expect(input).toHaveValue(""));
		expect(await screen.findByText(/in keychain/i)).toBeInTheDocument();
	});

	it("re-runs claude detection and shows the found path", async () => {
		mockSettingsIpc();
		const user = userEvent.setup();
		renderPage();

		expect(await screen.findByText(/not found on path/i)).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: /re-run detection/i }));
		expect(await screen.findByText("/opt/homebrew/bin/claude")).toBeInTheDocument();
	});
});
