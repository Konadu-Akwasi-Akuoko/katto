import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProjectPeek } from "@/features/planner/peek/project-peek";
import type { ProjectDetail } from "@/lib/ipc/projects";
import { useUiStore } from "@/stores/ui";

const detail: ProjectDetail = {
	project: {
		slug: "nvme-deep-dive-2026-07-01",
		title: "NVMe deep dive",
		root_path: "/studio/Projects/nvme-deep-dive-2026-07-01",
		status: "editing",
		target_nle: "resolve",
		shoot_date: "2026-07-03",
		publish_date: null,
		created_at: "2026-07-01",
		last_touched_at: "2026-07-05",
	},
	manifest_error: null,
	freshness: [
		{ subfolder: "footage", file_count: 12, latest_mtime: "2026-07-05" },
	],
};

const invalidManifestDetail: ProjectDetail = {
	...detail,
	manifest_error: "missing required field `status`",
	freshness: [],
};

function renderPeek(fixture: ProjectDetail = detail) {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "get_project") return fixture;
		if (cmd === "reveal_project_folder") return null;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<ProjectPeek />
		</QueryClientProvider>,
	);
	return { calls };
}

afterEach(() => {
	useUiStore.setState({
		peekSlug: null,
		surface: "dashboard",
		selectedProjectSlug: null,
	});
});

describe("ProjectPeek", () => {
	it("renders nothing until a slug is peeked", () => {
		renderPeek();
		expect(screen.queryByText("NVMe deep dive")).not.toBeInTheDocument();
	});

	it("shows the project's details when peeked", async () => {
		useUiStore.setState({ peekSlug: "nvme-deep-dive-2026-07-01" });
		renderPeek();
		expect(await screen.findByText("NVMe deep dive")).toBeInTheDocument();
		expect(screen.getByText("Editing")).toBeInTheDocument();
		expect(screen.getByText("footage")).toBeInTheDocument();
	});

	it("routes to full detail and closes on Open full detail", async () => {
		useUiStore.setState({ peekSlug: "nvme-deep-dive-2026-07-01" });
		renderPeek();
		await screen.findByText("NVMe deep dive");
		await userEvent.click(
			screen.getByRole("button", { name: /open full detail/i }),
		);
		expect(useUiStore.getState().surface).toBe("projects");
		expect(useUiStore.getState().selectedProjectSlug).toBe(
			"nvme-deep-dive-2026-07-01",
		);
		expect(useUiStore.getState().peekSlug).toBeNull();
	});

	it("shows the invalid-manifest state instead of dates and freshness", async () => {
		useUiStore.setState({ peekSlug: "nvme-deep-dive-2026-07-01" });
		renderPeek(invalidManifestDetail);
		expect(await screen.findByText("NVMe deep dive")).toBeInTheDocument();
		expect(
			screen.getByText(/invalid manifest: missing required field/i),
		).toBeInTheDocument();
		expect(screen.queryByText("Shoot")).not.toBeInTheDocument();
	});

	it("reveals the project folder on Reveal in Finder", async () => {
		useUiStore.setState({ peekSlug: "nvme-deep-dive-2026-07-01" });
		const { calls } = renderPeek();
		await screen.findByText("NVMe deep dive");
		await userEvent.click(
			screen.getByRole("button", { name: /reveal in finder/i }),
		);
		expect(calls).toHaveBeenCalledWith("reveal_project_folder", {
			slug: "nvme-deep-dive-2026-07-01",
			subfolder: null,
		});
	});
});
