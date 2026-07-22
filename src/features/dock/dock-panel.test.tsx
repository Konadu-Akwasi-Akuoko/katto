import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/lib/ipc/bindings.gen";
import { useUiStore } from "@/stores/ui";
import { DockPanel } from "./dock-panel";

function makeSession(id: string, label: string): SessionInfo {
	return {
		id,
		label,
		cwd: "/x",
		started_at: "2026-07-22 08:00:00",
		idle_since_secs: null,
		state: { kind: "running" },
	};
}

function renderPanel(sessions: SessionInfo[]) {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "list_sessions") return sessions;
		if (cmd === "set_dock_focus") return null;
		if (cmd === "spawn_session") return "spawned-id";
		if (cmd === "get_settings")
			return {
				studio_root: "/studio",
				default_nle: null,
				idle_reap_minutes: 5,
				onboarding_complete: true,
				claude_path: null,
				planner_model: "claude-sonnet-5",
				keys_present: { elevenlabs: false, anthropic: false },
			};
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<DockPanel />
		</QueryClientProvider>,
	);
	return { calls };
}

describe("DockPanel", () => {
	beforeEach(() => {
		useUiStore.setState({ dockOpen: false, activeSessionId: null });
	});

	it("renders nothing while closed", () => {
		renderPanel([]);
		expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
		expect(screen.queryByText("Claude")).not.toBeInTheDocument();
	});

	it("shows the tab strip when open with sessions", async () => {
		useUiStore.setState({ dockOpen: true, activeSessionId: "a" });
		renderPanel([makeSession("a", "ideas: nightly"), makeSession("b", "vfx")]);
		expect(await screen.findByRole("tablist")).toBeInTheDocument();
		expect(screen.getByRole("tab", { name: /ideas: nightly/ })).toBeVisible();
	});

	it("spawns a session from the New session button", async () => {
		const user = userEvent.setup();
		useUiStore.setState({ dockOpen: true });
		const { calls } = renderPanel([]);
		await user.click(
			await screen.findByRole("button", { name: "New session" }),
		);
		await waitFor(() =>
			expect(calls).toHaveBeenCalledWith(
				"spawn_session",
				expect.objectContaining({
					task: expect.objectContaining({ cwd: "/studio" }),
				}),
			),
		);
	});

	it("hides via the hide button", async () => {
		const user = userEvent.setup();
		useUiStore.setState({ dockOpen: true });
		renderPanel([]);
		await user.click(await screen.findByRole("button", { name: "Hide" }));
		expect(useUiStore.getState().dockOpen).toBe(false);
	});
});
