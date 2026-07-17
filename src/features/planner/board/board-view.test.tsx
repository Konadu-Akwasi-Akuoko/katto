import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BoardView } from "@/features/planner/board/board-view";
import type { DriveStatus } from "@/lib/ipc/drive";
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

const MOUNTED: DriveStatus = {
	mounted: true,
	path: "/Volumes/Studio",
	free_gb: 512,
};

function renderBoard(
	initial: Project[] = boardFixture,
	drive: DriveStatus = MOUNTED,
) {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "list_projects") return initial;
		if (cmd === "get_drive_status") return drive;
		if (cmd === "set_project_status") return null;
		if (cmd === "set_project_priority") return null;
		if (cmd === "trash_project") return null;
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

	it("opens the peek from the context menu", async () => {
		renderBoard();
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: await screen.findByText("NVMe deep dive"),
		});
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Open" }),
		);
		expect(useUiStore.getState().peekSlug).toBe("nvme-deep-dive-2026-07-08");
	});

	it("sets a priority from the context menu", async () => {
		const { calls } = renderBoard();
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: await screen.findByText("NVMe deep dive"),
		});
		await userEvent.click(
			await screen.findByRole("menuitem", { name: /set priority/i }),
		);
		await userEvent.click(
			await screen.findByRole("menuitemradio", { name: "High" }),
		);
		expect(calls).toHaveBeenCalledWith("set_project_priority", {
			slug: "nvme-deep-dive-2026-07-08",
			priority: "high",
		});
	});

	it("moves a project's status from the context menu", async () => {
		const { calls } = renderBoard();
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: await screen.findByText("Why RAID is dead"),
		});
		await userEvent.click(
			await screen.findByRole("menuitem", { name: /move to/i }),
		);
		await userEvent.click(
			await screen.findByRole("menuitemradio", { name: "Editing" }),
		);
		expect(calls).toHaveBeenCalledWith("set_project_status", {
			slug: "why-raid-is-dead-2026-07-07",
			status: "editing",
		});
	});

	it("won't move a project to the status it is already in", async () => {
		renderBoard();
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: await screen.findByText("Why RAID is dead"),
		});
		await userEvent.click(
			await screen.findByRole("menuitem", { name: /move to/i }),
		);
		expect(
			await screen.findByRole("menuitemradio", { name: "Idea" }),
		).toHaveAttribute("aria-disabled", "true");
	});

	it("disables the mutating actions when the studio drive is unmounted", async () => {
		const { calls } = renderBoard(boardFixture, {
			mounted: false,
			path: null,
			free_gb: null,
		});
		await screen.findByText("NVMe deep dive");
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: screen.getByText("NVMe deep dive"),
		});
		for (const name of [/set priority/i, /^move to$/i, /^delete$/i]) {
			expect(await screen.findByRole("menuitem", { name })).toHaveAttribute(
				"aria-disabled",
				"true",
			);
		}
		expect(screen.getByRole("menuitem", { name: "Open" })).not.toHaveAttribute(
			"aria-disabled",
			"true",
		);
		// A disabled trigger must not merely look disabled: activating it opens
		// no submenu, so no write can be reached from here at all.
		await userEvent.click(
			screen.getByRole("menuitem", { name: /set priority/i }),
		);
		expect(
			screen.queryByRole("menuitemradio", { name: "High" }),
		).not.toBeInTheDocument();
		expect(calls).not.toHaveBeenCalledWith(
			"set_project_priority",
			expect.anything(),
		);
	});

	it("trashes a project only after the confirm", async () => {
		const { calls } = renderBoard();
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: await screen.findByText("NVMe deep dive"),
		});
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Delete" }),
		);
		expect(calls).not.toHaveBeenCalledWith("trash_project", expect.anything());

		await userEvent.click(
			await screen.findByRole("button", { name: /move to trash/i }),
		);
		expect(calls).toHaveBeenCalledWith("trash_project", {
			slug: "nvme-deep-dive-2026-07-08",
		});
	});

	it("does not trash the project when the confirm is cancelled", async () => {
		const { calls } = renderBoard();
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: await screen.findByText("NVMe deep dive"),
		});
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Delete" }),
		);
		await userEvent.click(
			await screen.findByRole("button", { name: "Cancel" }),
		);

		expect(
			screen.queryByRole("button", { name: /move to trash/i }),
		).not.toBeInTheDocument();
		expect(calls).not.toHaveBeenCalledWith("trash_project", expect.anything());
		expect(screen.getByText("NVMe deep dive")).toBeInTheDocument();
	});

	it("leaves the card in place when the Trash fails", async () => {
		mockIPC((cmd) => {
			if (cmd === "list_projects") return boardFixture;
			if (cmd === "get_drive_status") return MOUNTED;
			if (cmd === "trash_project") throw new Error("permission denied");
			throw new Error(`unexpected command: ${cmd}`);
		});
		renderWithClient();
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: await screen.findByText("NVMe deep dive"),
		});
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Delete" }),
		);
		await userEvent.click(
			await screen.findByRole("button", { name: /move to trash/i }),
		);
		expect(await screen.findByText("NVMe deep dive")).toBeInTheDocument();
	});

	it("closes the peek when the peeked project is trashed", async () => {
		const { calls } = renderBoard();
		await userEvent.click(await screen.findByText("NVMe deep dive"));
		expect(useUiStore.getState().peekSlug).toBe("nvme-deep-dive-2026-07-08");
		await userEvent.pointer({
			keys: "[MouseRight]",
			target: screen.getByText("NVMe deep dive"),
		});
		await userEvent.click(
			await screen.findByRole("menuitem", { name: "Delete" }),
		);
		await userEvent.click(
			await screen.findByRole("button", { name: /move to trash/i }),
		);
		await waitFor(() => expect(useUiStore.getState().peekSlug).toBeNull());
		expect(calls).toHaveBeenCalledWith("trash_project", {
			slug: "nvme-deep-dive-2026-07-08",
		});
	});
});
