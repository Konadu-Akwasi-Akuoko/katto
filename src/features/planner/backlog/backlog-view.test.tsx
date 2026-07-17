import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { BacklogView } from "@/features/planner/backlog/backlog-view";
import type { Idea } from "@/lib/ipc/ideas";
import { backlogFixture } from "@/test/fixtures/ideas";

function renderBacklog(initial: Idea[] = backlogFixture) {
	let ideas = [...initial];
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "list_ideas") return ideas;
		if (cmd === "discard_idea") {
			const { id } = payload as { id: string };
			ideas = ideas.filter((idea) => idea.id !== id);
			return null;
		}
		if (cmd === "promote_idea") {
			const { id } = payload as { id: string };
			ideas = ideas.filter((idea) => idea.id !== id);
			return { slug: `${id}-2026-07-09` };
		}
		if (cmd === "create_idea") return initial[0];
		if (cmd === "update_idea") return initial[0];
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<BacklogView />
		</QueryClientProvider>,
	);
	return { calls };
}

function rowFor(title: string): HTMLElement {
	const node = screen.getByText(title).closest("li");
	if (!node) throw new Error(`no row for ${title}`);
	return node;
}

describe("BacklogView", () => {
	it("renders each backlog idea", async () => {
		renderBacklog();
		expect(await screen.findByText("NVMe deep dive")).toBeInTheDocument();
		expect(screen.getByText("Why RAID is dead")).toBeInTheDocument();
	});

	it("discards an idea and removes its row", async () => {
		const user = userEvent.setup();
		const { calls } = renderBacklog();
		await screen.findByText("NVMe deep dive");

		await user.click(
			within(rowFor("NVMe deep dive")).getByRole("button", {
				name: /discard/i,
			}),
		);

		expect(calls).toHaveBeenCalledWith("discard_idea", { id: "idea-1" });
		await waitFor(() =>
			expect(screen.queryByText("NVMe deep dive")).not.toBeInTheDocument(),
		);
		expect(screen.getByText("Why RAID is dead")).toBeInTheDocument();
	});

	it("promotes an idea and removes its row", async () => {
		const user = userEvent.setup();
		const { calls } = renderBacklog();
		await screen.findByText("Why RAID is dead");

		await user.click(
			within(rowFor("Why RAID is dead")).getByRole("button", {
				name: /promote/i,
			}),
		);

		expect(calls).toHaveBeenCalledWith("promote_idea", { id: "idea-2" });
		await waitFor(() =>
			expect(screen.queryByText("Why RAID is dead")).not.toBeInTheDocument(),
		);
	});

	it("shows an empty state when the backlog is clear", async () => {
		renderBacklog([]);
		expect(
			await screen.findByText(/nothing in the backlog/i),
		).toBeInTheDocument();
	});
});
