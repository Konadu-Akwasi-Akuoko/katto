import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BoardView } from "@/features/planner/board/board-view";
import type { Project } from "@/lib/ipc/projects";
import { projectsKeys } from "@/lib/ipc/projects";
import { useUiStore } from "@/stores/ui";
import { boardFixture, project } from "@/test/fixtures/projects";

const COLUMN_LABELS = ["Idea", "Shooting", "Editing", "Published"];

const UNMOUNTED = "studio root is not mounted: /Volumes/Studio";

function renderWithClient() {
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<BoardView />
		</QueryClientProvider>,
	);
	return { client };
}

function renderBoard(initial: Project[] = boardFixture) {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "list_projects") return initial;
		if (cmd === "set_project_status") return null;
		throw new Error(`unexpected command: ${cmd}`);
	});
	return { calls, ...renderWithClient() };
}

/**
 * Settle a refetch all the way into the DOM. Awaiting the refetch promise only
 * settles the fetch: Query hands observer results to React through
 * notifyManager, whose default scheduler is a `setTimeout(_, 0)`. Without
 * crossing that macrotask the DOM is still the pre-refetch render, and any
 * assertion about the result is vacuous.
 */
async function flushRefetch(client: QueryClient): Promise<void> {
	await act(async () => {
		await client.refetchQueries({ queryKey: projectsKeys.all });
		await new Promise((resolve) => setTimeout(resolve, 0));
	});
}

/** The card element wrapping a title — dnd-kit's attributes give it role="button". */
function cardFor(title: string): HTMLElement {
	const card = screen.getByText(title).closest('[role="button"]');
	if (!(card instanceof HTMLElement)) throw new Error(`no card for ${title}`);
	return card;
}

function columnFor(label: string): HTMLElement {
	const heading = screen.getByRole("heading", { name: label });
	const section = heading.closest("section");
	if (!section) throw new Error(`no column section for ${label}`);
	return section;
}

describe("BoardView", () => {
	afterEach(() => useUiStore.setState({ peekSlug: null }));

	it("renders the four workflow columns", async () => {
		renderBoard();
		await screen.findByText("NVMe deep dive");
		for (const label of COLUMN_LABELS) {
			expect(screen.getByRole("heading", { name: label })).toBeInTheDocument();
		}
	});

	it("places each project card under its status column", async () => {
		renderBoard();
		await screen.findByText("NVMe deep dive");
		expect(
			within(columnFor("Editing")).getByText("NVMe deep dive"),
		).toBeInTheDocument();
		expect(
			within(columnFor("Idea")).getByText("Why RAID is dead"),
		).toBeInTheDocument();
		expect(
			within(columnFor("Shooting")).getByText("Thermal throttling"),
		).toBeInTheDocument();
		expect(
			within(columnFor("Published")).getByText("RAID rebuild diary"),
		).toBeInTheDocument();
	});

	it("shows the board's error state instead of four empty columns", async () => {
		mockIPC(() => {
			throw new Error(UNMOUNTED);
		});
		renderWithClient();
		expect(
			await screen.findByText(/couldn't load your projects/i),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("heading", { name: "Idea" }),
		).not.toBeInTheDocument();
	});

	it("renders the empty columns for a studio with no projects", async () => {
		renderBoard([]);
		for (const label of COLUMN_LABELS) {
			expect(
				await screen.findByRole("heading", { name: label }),
			).toBeInTheDocument();
		}
		expect(
			screen.queryByText(/couldn't load your projects/i),
		).not.toBeInTheDocument();
	});

	it("keeps a loaded board on screen when a background refetch fails", async () => {
		let listed = 0;
		mockIPC((cmd) => {
			if (cmd !== "list_projects")
				throw new Error(`unexpected command: ${cmd}`);
			listed += 1;
			if (listed > 1) throw new Error(UNMOUNTED);
			return boardFixture;
		});
		const { client } = renderWithClient();
		await screen.findByText("NVMe deep dive");

		await flushRefetch(client);

		expect(listed).toBe(2);
		expect(client.getQueryState(projectsKeys.all)?.status).toBe("error");
		expect(screen.getByText("NVMe deep dive")).toBeInTheDocument();
		expect(
			screen.getByRole("heading", { name: "Editing" }),
		).toBeInTheDocument();
		expect(
			screen.queryByText(/couldn't load your projects/i),
		).not.toBeInTheDocument();
	});

	it("opens the peek when a card is clicked", async () => {
		renderBoard();
		await userEvent.click(await screen.findByText("NVMe deep dive"));
		expect(useUiStore.getState().peekSlug).toBe("nvme-deep-dive-2026-07-08");
	});

	it("shows a priority tab only on prioritised cards", async () => {
		renderBoard([
			project({ slug: "hot-2026-07-01", title: "Hot one", priority: "high" }),
			project({ slug: "cold-2026-07-01", title: "Cold one", priority: "none" }),
		]);
		await screen.findByText("Hot one");
		// Scoped per card, and the negative asserts on a label that *can* render:
		// "None" has no appearance entry, so querying for it would pass against
		// any implementation, including one with no tab guard at all.
		expect(within(cardFor("Hot one")).getByText("High")).toBeInTheDocument();
		expect(
			within(cardFor("Cold one")).queryByText("High"),
		).not.toBeInTheDocument();
	});
});
