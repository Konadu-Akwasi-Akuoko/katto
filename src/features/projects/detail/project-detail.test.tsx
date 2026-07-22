import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProjectDetail } from "@/features/projects/detail/project-detail";
import type { ProjectDetail as ProjectDetailPayload } from "@/lib/ipc/projects";
import { useUiStore } from "@/stores/ui";
import { project, projectDetail } from "@/test/fixtures/projects";

const validDetail = projectDetail(
	project({
		slug: "nvme-deep-dive-2026-07-08",
		title: "NVMe deep dive",
		status: "editing",
	}),
);

function renderDetail(detail: ProjectDetailPayload = validDetail) {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "get_project") return detail;
		if (cmd === "reveal_project_folder") return null;
		if (cmd === "set_project_dates") return null;
		if (cmd === "latest_thumbnail") return null;
		if (cmd === "watch_thumbnails") return null;
		if (cmd === "unwatch_thumbnails") return null;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<ProjectDetail slug={detail.project.slug} />
		</QueryClientProvider>,
	);
	return { calls };
}

describe("ProjectDetail", () => {
	beforeEach(() => {
		useUiStore.setState({
			surface: "projects",
			selectedProjectSlug: "nvme-deep-dive-2026-07-08",
		});
	});

	it("renders the project title and freshness grid", async () => {
		renderDetail();
		expect(await screen.findByText("NVMe deep dive")).toBeInTheDocument();
		expect(screen.getByText("footage")).toBeInTheDocument();
	});

	it("badges an invalid manifest with its raw error", async () => {
		renderDetail(
			projectDetail(project({ slug: "broken-2026-07-08", title: "Broken" }), {
				manifest_error: "slug mismatch: expected broken-2026-07-08",
			}),
		);
		expect(await screen.findByText(/invalid manifest/i)).toBeInTheDocument();
		expect(
			screen.getByText(/slug mismatch: expected broken-2026-07-08/),
		).toBeInTheDocument();
	});

	it("reveals a subfolder in Finder with the right subfolder", async () => {
		const user = userEvent.setup();
		const { calls } = renderDetail();
		await user.click(
			await screen.findByRole("button", { name: /reveal footage in finder/i }),
		);
		expect(calls).toHaveBeenCalledWith("reveal_project_folder", {
			slug: "nvme-deep-dive-2026-07-08",
			subfolder: "footage",
		});
	});
});
