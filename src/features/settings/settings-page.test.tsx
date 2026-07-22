import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { SettingsPage } from "@/features/settings/settings-page";
import { settingsFixture } from "@/test/fixtures/settings";

function mockSettingsIpc(
	opts: { captureShortcut?: string; captureShortcutTaken?: boolean } = {},
) {
	const calls: [string, unknown][] = [];
	let autostart = false;
	let elevenlabsPresent = false;
	let claudePath: string | null = null;
	let captureShortcut =
		opts.captureShortcut ?? settingsFixture.capture_shortcut;

	mockIPC((cmd, args) => {
		calls.push([cmd, args]);
		switch (cmd) {
			case "get_settings":
				return {
					...settingsFixture,
					studio_root: "/Volumes/Studio",
					onboarding_complete: true,
					claude_path: claudePath,
					capture_shortcut: captureShortcut,
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
			case "set_capture_shortcut": {
				if (opts.captureShortcutTaken) {
					// A non-Error throw reaches the generated runtime as the tagged
					// payload, which unwrap() rethrows as IpcError.
					throw {
						kind: "shortcut_unavailable",
						message: "'ctrl+alt+p' is unavailable",
					};
				}
				captureShortcut = (args as { accel: string }).accel;
				return { ...settingsFixture, capture_shortcut: captureShortcut };
			}
			default:
				if (cmd.startsWith("plugin:event|")) return 1;
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

		const toggle = await screen.findByRole("switch", {
			name: /launch at login/i,
		});
		expect(toggle).not.toBeChecked();
		await user.click(toggle);

		await waitFor(() =>
			expect(calls).toContainEqual(["set_autostart", { enabled: true }]),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("switch", { name: /launch at login/i }),
			).toBeChecked(),
		);
	});

	it("stores a key write-only and reflects presence", async () => {
		mockSettingsIpc();
		const user = userEvent.setup();
		renderPage();

		const input = await screen.findByLabelText("ElevenLabs API key");
		await user.type(input, "xi-secret");
		await user.click(
			screen.getByRole("button", { name: "Save ElevenLabs key" }),
		);

		await waitFor(() => expect(input).toHaveValue(""));
		expect(await screen.findByText(/in keychain/i)).toBeInTheDocument();
	});

	it("re-runs claude detection and shows the found path", async () => {
		mockSettingsIpc();
		const user = userEvent.setup();
		renderPage();

		expect(await screen.findByText(/not found on path/i)).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: /re-run detection/i }));
		expect(
			await screen.findByText("/opt/homebrew/bin/claude"),
		).toBeInTheDocument();
	});

	it("shows the current capture shortcut as glyphs", async () => {
		mockSettingsIpc();
		renderPage();

		expect(await screen.findByText("⌥")).toBeInTheDocument();
		expect(screen.getByText("⌘")).toBeInTheDocument();
		expect(screen.getByText("K")).toBeInTheDocument();
	});

	it("rebinds via a recorded combo", async () => {
		const calls = mockSettingsIpc();
		const user = userEvent.setup();
		renderPage();

		await user.click(await screen.findByRole("button", { name: /rebind/i }));
		expect(screen.getByText(/press shortcut/i)).toBeInTheDocument();
		fireEvent.keyDown(window, { code: "KeyJ", metaKey: true, altKey: true });

		await waitFor(() =>
			expect(calls).toContainEqual([
				"set_capture_shortcut",
				{ accel: "alt+cmd+j" },
			]),
		);
		expect(await screen.findByText("J")).toBeInTheDocument();
	});

	it("esc cancels recording without invoking", async () => {
		const calls = mockSettingsIpc();
		const user = userEvent.setup();
		renderPage();

		await user.click(await screen.findByRole("button", { name: /rebind/i }));
		fireEvent.keyDown(window, { code: "Escape" });

		expect(await screen.findByText("⌥")).toBeInTheDocument();
		expect(
			calls.filter(([cmd]) => cmd === "set_capture_shortcut"),
		).toHaveLength(0);
	});

	it("keeps the old binding shown when the shortcut is unavailable", async () => {
		const calls = mockSettingsIpc({ captureShortcutTaken: true });
		const user = userEvent.setup();
		renderPage();

		await user.click(await screen.findByRole("button", { name: /rebind/i }));
		fireEvent.keyDown(window, { code: "KeyP", ctrlKey: true, altKey: true });

		await waitFor(() =>
			expect(calls).toContainEqual([
				"set_capture_shortcut",
				{ accel: "ctrl+alt+p" },
			]),
		);
		expect(await screen.findByText("⌥")).toBeInTheDocument();
		expect(screen.getByText("K")).toBeInTheDocument();
		expect(screen.queryByText(/press shortcut/i)).not.toBeInTheDocument();
	});

	it("reset invokes with the default", async () => {
		const calls = mockSettingsIpc({ captureShortcut: "ctrl+alt+p" });
		const user = userEvent.setup();
		renderPage();

		await user.click(await screen.findByRole("button", { name: /reset/i }));

		await waitFor(() =>
			expect(calls).toContainEqual([
				"set_capture_shortcut",
				{ accel: "alt+cmd+k" },
			]),
		);
	});
});
