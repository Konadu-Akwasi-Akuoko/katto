import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { ProjectsList } from "@/features/projects/list/projects-list";
import type { Project } from "@/lib/ipc/projects";
import { useUiStore } from "@/stores/ui";
import { project } from "@/test/fixtures/projects";

const listFixture: Project[] = [
	project({
		slug: "nvme-deep-dive-2026-07-08",
		title: "NVMe deep dive",
		status: "editing",
		shoot_date: "2026-07-10",
	}),
	project({
		slug: "why-raid-is-dead-2026-07-07",
		title: "Why RAID is dead",
		status: "idea",
	}),
];

function renderList(projects: Project[] = listFixture) {
	mockIPC((cmd) => {
		if (cmd === "list_projects") return projects;
		if (cmd === "list_latest_thumbnails") return [];
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return render(
		<QueryClientProvider client={client}>
			<ProjectsList />
		</QueryClientProvider>,
	);
}

describe("ProjectsList", () => {
	beforeEach(() => {
		useUiStore.setState({ surface: "projects", selectedProjectSlug: null });
	});

	it("renders a row per project with its title and status", async () => {
		renderList();
		expect(await screen.findByText("NVMe deep dive")).toBeInTheDocument();
		expect(screen.getByText("Why RAID is dead")).toBeInTheDocument();
		expect(screen.getByText("Editing")).toBeInTheDocument();
	});

	it("selects a project when its row is clicked", async () => {
		const user = userEvent.setup();
		renderList();
		await user.click(await screen.findByText("NVMe deep dive"));
		expect(useUiStore.getState().selectedProjectSlug).toBe(
			"nvme-deep-dive-2026-07-08",
		);
	});

	it("shows a plain empty state with no projects", async () => {
		renderList([]);
		expect(await screen.findByText(/No projects yet/i)).toBeInTheDocument();
	});

	it("clears the promote-arrival flag when its animation ends", async () => {
		useUiStore.setState({ justPromotedSlug: "nvme-deep-dive-2026-07-08" });
		renderList();
		const row = await screen.findByText("NVMe deep dive");
		const button = row.closest("button");
		if (!button) throw new Error("no project row button");
		// jsdom lacks style.animation, so React falls back to listening for
		// the webkit-prefixed event name in tests
		const event = new Event("webkitAnimationEnd", { bubbles: true });
		Object.defineProperty(event, "animationName", {
			value: "promote-arrival",
		});
		fireEvent(button, event);
		expect(useUiStore.getState().justPromotedSlug).toBeNull();
	});
});
