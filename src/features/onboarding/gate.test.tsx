import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { OnboardingGate } from "@/features/onboarding/gate";
import type { Settings } from "@/lib/ipc/settings";
import { settingsFixture } from "@/test/fixtures/settings";

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
			await screen.findByRole("heading", {
				name: /where does your footage live/i,
			}),
		).toBeInTheDocument();
	});
});
