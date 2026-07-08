import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import type { Settings } from "@/lib/ipc/bindings.gen";
import { OnboardingGate } from "@/features/onboarding/gate";

const settingsFixture: Settings = {
	studio_root: null,
	default_nle: null,
	idle_reap_minutes: 10,
	onboarding_complete: false,
	claude_path: null,
	keys_present: { elevenlabs: false, anthropic: false },
};

function renderGate(settings: Settings) {
	mockIPC((cmd) => {
		if (cmd === "get_settings") return settings;
		if (cmd === "detect_claude") return null;
		throw new Error(`unexpected command ${cmd}`);
	});
	const qc = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	});
	return render(
		<QueryClientProvider client={qc}>
			<OnboardingGate>
				<div>the app</div>
			</OnboardingGate>
		</QueryClientProvider>,
	);
}

describe("OnboardingGate", () => {
	it("renders the app once onboarding is complete", async () => {
		renderGate({ ...settingsFixture, onboarding_complete: true });
		expect(await screen.findByText("the app")).toBeInTheDocument();
	});

	it("shows the wizard on first run", async () => {
		renderGate(settingsFixture);
		expect(
			await screen.findByRole("heading", { name: /where does your footage live/i }),
		).toBeInTheDocument();
	});
});
