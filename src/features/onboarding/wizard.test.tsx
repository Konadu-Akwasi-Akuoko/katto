import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { OnboardingWizard } from "@/features/onboarding/wizard";
import { settingsFixture } from "@/test/fixtures/settings";

const goodRoot = {
	path: "/Volumes/Studio",
	writable: true,
	on_boot_volume: false,
	free_gb: 500,
	low_free_space: false,
};

const savedSettings = { ...settingsFixture, studio_root: "/Volumes/Studio" };

function mockOnboardingIpc(
	overrides: Record<string, (args: unknown) => unknown> = {},
) {
	const calls: string[] = [];
	mockIPC((cmd, args) => {
		calls.push(cmd);
		const handler = overrides[cmd];
		if (handler) return handler(args);
		switch (cmd) {
			case "pick_studio_root":
				return goodRoot;
			case "set_settings":
				return savedSettings;
			case "store_key":
			case "complete_onboarding":
				return null;
			case "detect_claude":
				return "/opt/homebrew/bin/claude";
			default:
				throw new Error(`unexpected command ${cmd}`);
		}
	});
	return calls;
}

function renderWizard() {
	const qc = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	});
	return render(
		<QueryClientProvider client={qc}>
			<OnboardingWizard />
		</QueryClientProvider>,
	);
}

describe("OnboardingWizard", () => {
	it("walks root → key → claude and completes", async () => {
		const calls = mockOnboardingIpc();
		const user = userEvent.setup();
		renderWizard();

		expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
		await user.click(screen.getByRole("button", { name: "Choose folder…" }));
		expect(await screen.findByText("/Volumes/Studio")).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: "Continue" }));

		await user.click(
			await screen.findByRole("button", { name: "Skip for now" }),
		);

		expect(
			await screen.findByText("/opt/homebrew/bin/claude"),
		).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: "Finish" }));
		await waitFor(() => expect(calls).toContain("complete_onboarding"));
		expect(calls).toContain("set_settings");
	});

	it("keeps continue disabled for an unwritable root", async () => {
		mockOnboardingIpc({
			pick_studio_root: () => ({
				path: "/locked",
				writable: false,
				on_boot_volume: true,
				free_gb: 10,
				low_free_space: true,
			}),
		});
		const user = userEvent.setup();
		renderWizard();

		await user.click(screen.getByRole("button", { name: "Choose folder…" }));
		expect(await screen.findByText(/can't write/i)).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
	});

	it("stores the key and relabels skip to continue", async () => {
		const calls = mockOnboardingIpc();
		const user = userEvent.setup();
		renderWizard();

		await user.click(screen.getByRole("button", { name: "Choose folder…" }));
		await screen.findByText("/Volumes/Studio");
		await user.click(screen.getByRole("button", { name: "Continue" }));

		await user.type(await screen.findByLabelText("API key"), "xi-secret");
		await user.click(screen.getByRole("button", { name: "Save key" }));
		expect(
			await screen.findByText(/stored in your keychain/i),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Continue" }),
		).toBeInTheDocument();
		await waitFor(() => expect(calls).toContain("store_key"));
	});
});
