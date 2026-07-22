import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockConvertFileSrc, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VfxEffect } from "@/lib/ipc/vfx";
import { useUiStore } from "@/stores/ui";
import { VfxCard } from "./vfx-card";

function renderCard(effects: VfxEffect[]) {
	const calls = vi.fn();
	mockConvertFileSrc("asset");
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "list_vfx_effects") return effects;
		if (cmd === "create_vfx_effect") return "vfx-session-1";
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<VfxCard slug="proj" />
		</QueryClientProvider>,
	);
	return { calls };
}

describe("VfxCard", () => {
	beforeEach(() => {
		useUiStore.setState({ dockOpen: false, activeSessionId: null });
	});

	it("renders the empty-state invitation", async () => {
		renderCard([]);
		expect(
			await screen.findByText(
				"No effects yet. New effect opens a Claude session in its folder.",
			),
		).toBeInTheDocument();
	});

	it("renders effect tiles with a preview video when a render exists", async () => {
		renderCard([
			{
				effect: "intro-glitch",
				path: "/p/assets/vfx/intro-glitch",
				renders: ["final.mp4", "v1.mp4"],
			},
			{ effect: "empty-one", path: "/p/assets/vfx/empty-one", renders: [] },
		]);
		expect(await screen.findByText("intro-glitch")).toBeInTheDocument();
		expect(screen.getByTestId("vfx-preview-intro-glitch")).toBeInTheDocument();
		expect(screen.getByText("no render yet")).toBeInTheDocument();
		expect(screen.getByText("2 renders")).toBeInTheDocument();
	});

	it("creates an effect from the dialog and opens the dock on its session", async () => {
		const user = userEvent.setup();
		const { calls } = renderCard([]);
		await user.click(await screen.findByRole("button", { name: "New effect" }));
		await user.type(screen.getByLabelText("Effect name"), "Intro Glitch");
		await user.click(screen.getByRole("button", { name: "Create" }));
		await waitFor(() =>
			expect(calls).toHaveBeenCalledWith("create_vfx_effect", {
				projectSlug: "proj",
				name: "Intro Glitch",
			}),
		);
		await waitFor(() => {
			expect(useUiStore.getState().dockOpen).toBe(true);
			expect(useUiStore.getState().activeSessionId).toBe("vfx-session-1");
		});
	});
});
