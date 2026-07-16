import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { BoardView } from "@/features/planner/board/board-view";
import type { Project } from "@/lib/ipc/projects";
import { boardFixture } from "@/test/fixtures/projects";

function renderBoard(initial: Project[] = boardFixture) {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "list_projects") return initial;
		if (cmd === "set_project_status") return null;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<BoardView />
		</QueryClientProvider>,
	);
	return { calls };
}

function columnFor(label: string): HTMLElement {
	const heading = screen.getByRole("heading", { name: label });
	const section = heading.closest("section");
	if (!section) throw new Error(`no column section for ${label}`);
	return section;
}

describe("BoardView", () => {
	it("renders the four workflow columns", async () => {
		renderBoard();
		await screen.findByText("NVMe deep dive");
		for (const label of ["Idea", "Shooting", "Editing", "Published"]) {
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
			throw new Error("studio root is not mounted: /Volumes/Studio");
		});
		const client = new QueryClient({
			defaultOptions: { queries: { retry: false } },
		});
		render(
			<QueryClientProvider client={client}>
				<BoardView />
			</QueryClientProvider>,
		);
		expect(
			await screen.findByText(/couldn't load your projects/i),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("heading", { name: "Idea" }),
		).not.toBeInTheDocument();
	});
});
