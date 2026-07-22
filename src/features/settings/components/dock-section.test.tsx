import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { settingsFixture } from "@/test/fixtures/settings";
import { DockSection } from "./dock-section";

function renderSection() {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "get_scheduler_state")
			return [
				{
					name: "nightly-curation",
					spec: "daily@02:30;catchup=20h",
					last_success_at: "2026-07-22 00:04:11",
					enabled: true,
				},
			];
		if (cmd === "run_scheduled_job_now") return null;
		if (cmd === "set_scheduled_job") return null;
		if (cmd === "set_settings") return settingsFixture;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<DockSection settings={settingsFixture} />
		</QueryClientProvider>,
	);
	return { calls };
}

describe("DockSection", () => {
	it("renders the dock controls with accessible labels", async () => {
		renderSection();
		expect(
			screen.getByLabelText("Idle sessions close after"),
		).toBeInTheDocument();
		expect(
			await screen.findByLabelText("Nightly curation"),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Run now" })).toBeInTheDocument();
		expect(
			screen.getByLabelText("Cut planning in the dock"),
		).toBeInTheDocument();
		expect(screen.getByLabelText("Discovery")).toBeInTheDocument();
	});

	it("shows the schedule time parsed from the spec", async () => {
		renderSection();
		await waitFor(() => {
			const time = screen.getByLabelText<HTMLInputElement>("Curation time");
			expect(time.value).toBe("02:30");
		});
	});

	it("runs curation now on the Run now button", async () => {
		const user = userEvent.setup();
		const { calls } = renderSection();
		await user.click(await screen.findByRole("button", { name: "Run now" }));
		await waitFor(() =>
			expect(calls).toHaveBeenCalledWith("run_scheduled_job_now", {
				name: "nightly-curation",
			}),
		);
	});

	it("disables curation via set_scheduled_job keeping the time", async () => {
		const user = userEvent.setup();
		const { calls } = renderSection();
		await user.click(await screen.findByLabelText("Nightly curation"));
		await waitFor(() =>
			expect(calls).toHaveBeenCalledWith("set_scheduled_job", {
				name: "nightly-curation",
				hour: 2,
				minute: 30,
				enabled: false,
			}),
		);
	});
});
