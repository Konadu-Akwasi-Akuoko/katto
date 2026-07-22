import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { BrowserState } from "@/lib/ipc/browser";
import { BrowserSurface } from "./browser-surface";

function makeState(): BrowserState {
	return {
		tabs: [
			{
				id: 1,
				title: "elements.envato.com",
				url: "https://elements.envato.com/",
				can_go_back: false,
				can_go_forward: false,
			},
			{
				id: 2,
				title: "example.com",
				url: "https://example.com/",
				can_go_back: true,
				can_go_forward: false,
			},
		],
		active: 2,
	};
}

function renderSurface(state: BrowserState) {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "browser_state") return state;
		if (cmd === "browser_open_tab") return 1;
		if (cmd === "active_asset_project") return "sprint-recap";
		if (cmd === "list_projects") return [];
		return null;
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<BrowserSurface />
		</QueryClientProvider>,
	);
	return { calls };
}

describe("BrowserSurface", () => {
	it("renders the tab strip and marks the active tab", async () => {
		renderSurface(makeState());
		const tabs = await screen.findAllByRole("tab");
		expect(tabs).toHaveLength(2);
		expect(tabs[1]).toHaveAttribute("aria-selected", "true");
		expect(tabs[0]).toHaveAttribute("aria-selected", "false");
	});

	it("navigates when a bare host is entered in the address bar", async () => {
		const { calls } = renderSurface(makeState());
		await screen.findAllByRole("tab");
		const input = screen.getByRole("textbox", { name: "Address" });
		fireEvent.change(input, { target: { value: "example.com" } });
		fireEvent.keyDown(input, { key: "Enter" });
		await waitFor(() => {
			expect(calls).toHaveBeenCalledWith("browser_navigate", {
				tabId: 2,
				url: "https://example.com",
			});
		});
	});

	it("opens the Envato default tab when no tabs exist", async () => {
		const { calls } = renderSurface({ tabs: [], active: null });
		await waitFor(() => {
			expect(calls).toHaveBeenCalledWith("browser_open_tab", { url: null });
		});
	});
});
