import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import {
	act,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { BrowserState } from "@/lib/ipc/browser";
import { useDownloadsStore } from "@/stores/downloads";
import { useUiStore } from "@/stores/ui";
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
			{
				id: 3,
				title: "New tab",
				url: null,
				can_go_back: false,
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

function lastVisibleCall(calls: ReturnType<typeof vi.fn>): boolean | undefined {
	const visible = calls.mock.calls.filter(
		([cmd]) => cmd === "browser_set_visible",
	);
	const last = visible[visible.length - 1];
	return (last?.[1] as { visible: boolean } | undefined)?.visible;
}

describe("BrowserSurface", () => {
	afterEach(() => {
		useUiStore.setState({ dockOpen: false, paletteOpen: false });
		useDownloadsStore.setState({ rows: [], needsProject: null });
	});

	it("renders the tab strip and marks the active tab", async () => {
		renderSurface(makeState());
		const tabs = await screen.findAllByRole("tab");
		expect(tabs).toHaveLength(3);
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

	it("renders the start page when there are no tabs", async () => {
		const { calls } = renderSurface({ tabs: [], active: null });
		expect(
			await screen.findByRole("button", { name: "Envato Elements" }),
		).toBeInTheDocument();
		expect(calls).not.toHaveBeenCalledWith("browser_open_tab", { url: null });
	});

	it("renders the start page for a url-less active tab", async () => {
		const state = makeState();
		state.active = 3;
		renderSurface(state);
		expect(
			await screen.findByRole("button", { name: "Freesound" }),
		).toBeInTheDocument();
	});

	it("reports bounds before visibility on mount", async () => {
		const { calls } = renderSurface(makeState());
		await screen.findAllByRole("tab");
		const cmds = calls.mock.calls.map(([cmd]) => cmd as string);
		const bounds = cmds.indexOf("browser_set_bounds");
		const visible = cmds.indexOf("browser_set_visible");
		expect(bounds).toBeGreaterThanOrEqual(0);
		expect(visible).toBeGreaterThan(bounds);
	});

	it("hides the webview while the dock overlay is open, restores on close", async () => {
		const { calls } = renderSurface(makeState());
		await screen.findAllByRole("tab");
		await waitFor(() => expect(lastVisibleCall(calls)).toBe(true));
		act(() => useUiStore.setState({ dockOpen: true }));
		await waitFor(() => expect(lastVisibleCall(calls)).toBe(false));
		act(() => useUiStore.setState({ dockOpen: false }));
		await waitFor(() => expect(lastVisibleCall(calls)).toBe(true));
	});

	// the switcher popover hangs over the content rect, and the native webview
	// paints above the DOM — this read is the whole reason `switcherOpen` lives
	// in the ui store rather than inside the switcher
	it("hides the webview while the surface switcher is open", async () => {
		const { calls } = renderSurface(makeState());
		await screen.findAllByRole("tab");
		await waitFor(() => expect(lastVisibleCall(calls)).toBe(true));
		act(() => useUiStore.setState({ switcherOpen: true }));
		await waitFor(() => expect(lastVisibleCall(calls)).toBe(false));
		act(() => useUiStore.setState({ switcherOpen: false }));
		await waitFor(() => expect(lastVisibleCall(calls)).toBe(true));
	});

	it("closing the last tab lands on the start page, not a respawn", async () => {
		const state = makeState();
		state.tabs = [state.tabs[0] as (typeof state.tabs)[number]];
		state.active = 1;
		const { calls } = renderSurface(state);
		const closeButtons = await screen.findAllByRole("button", {
			name: /close/i,
		});
		calls.mockClear();
		state.tabs = [];
		state.active = null;
		fireEvent.click(closeButtons[0] as HTMLElement);
		expect(
			await screen.findByRole(
				"button",
				{ name: "Envato Elements" },
				{
					timeout: 3000,
				},
			),
		).toBeInTheDocument();
		expect(calls).not.toHaveBeenCalledWith("browser_open_tab", { url: null });
	});

	it("hides the webview while the needs-project sheet is open", async () => {
		const { calls } = renderSurface(makeState());
		await screen.findAllByRole("tab");
		act(() =>
			useDownloadsStore
				.getState()
				.setNeedsProject({ id: "d1", filename: "dust.zip" }),
		);
		await waitFor(() => expect(lastVisibleCall(calls)).toBe(false));
	});
});
