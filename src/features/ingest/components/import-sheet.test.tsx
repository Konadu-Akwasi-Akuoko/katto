import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ImportSheet } from "@/features/ingest/components/import-sheet";
import type { DriveStatus } from "@/lib/ipc/drive";
import type { CardOffer } from "@/lib/ipc/ingest";
import type { Project } from "@/lib/ipc/projects";
import { useIngestSheetStore } from "@/stores/ingest-sheet";

const offer: CardOffer = {
	volume: "/Volumes/SONY",
	kind: "sony",
	total_bytes: 300,
	groups: [
		{
			label: "CLIP",
			clips: [
				{
					path: "PRIVATE/M4ROOT/CLIP/C0001.MP4",
					name: "C0001.MP4",
					size: 100,
					is_video: true,
					selected: true,
					duration_s: 12.5,
				},
				{
					path: "PRIVATE/M4ROOT/CLIP/C0001M01.XML",
					name: "C0001M01.XML",
					size: 5,
					is_video: false,
					selected: false,
					duration_s: null,
				},
				{
					path: "PRIVATE/M4ROOT/CLIP/C0002.MP4",
					name: "C0002.MP4",
					size: 200,
					is_video: true,
					selected: true,
					duration_s: null,
				},
			],
		},
	],
};

const project: Project = {
	slug: "nvme-deep-dive-2026-07-20",
	title: "NVMe deep dive",
	root_path: "/Volumes/Studio/Projects/nvme-deep-dive-2026-07-20",
	status: "shooting",
	target_nle: "resolve",
	priority: "none",
	shoot_date: "2026-07-21",
	publish_date: null,
	created_at: "2026-07-20T10:00:00Z",
	last_touched_at: null,
};

const MOUNTED: DriveStatus = {
	mounted: true,
	path: "/Volumes/Studio",
	free_gb: 512,
};

function renderSheet() {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "card_offer") return offer;
		if (cmd === "list_projects") return [project];
		if (cmd === "get_drive_status") return MOUNTED;
		if (cmd === "start_ingest")
			return {
				id: "job-1",
				kind: "ingest",
				label: "Import 2 clips",
				status: "queued",
				progress: 0,
				payload_json: null,
				error: null,
				created_at: "2026-07-22T10:00:00Z",
				started_at: null,
				finished_at: null,
			};
		if (cmd === "subscribe_job_progress") return null;
		if (typeof cmd === "string" && cmd.startsWith("plugin:event|")) return 1;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<ImportSheet />
		</QueryClientProvider>,
	);
	return { calls };
}

afterEach(() => {
	useIngestSheetStore.setState({ open: false });
});

describe("ImportSheet", () => {
	it("lists card clips grouped with videos pre-selected and sidecars listed deselected", async () => {
		useIngestSheetStore.setState({ open: true });
		renderSheet();

		expect(await screen.findByText("C0001.MP4")).toBeInTheDocument();
		expect(screen.getByText("C0002.MP4")).toBeInTheDocument();
		expect(
			await screen.findByRole("heading", { name: "Import 2 clips" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("checkbox", { name: "Import C0001.MP4" }),
		).toBeChecked();
		// Sidecars are listed but deselected (PRD: listed, not pre-selected).
		expect(screen.getByText("C0001M01.XML")).toBeInTheDocument();
		expect(
			screen.getByRole("checkbox", { name: "Import C0001M01.XML" }),
		).not.toBeChecked();
	});

	it("starts the ingest with the selected paths and target project", async () => {
		useIngestSheetStore.setState({ open: true });
		const { calls } = renderSheet();
		const user = userEvent.setup();

		// Deselect one clip, then import the remaining selection.
		await user.click(
			await screen.findByRole("checkbox", { name: "Import C0001.MP4" }),
		);
		await user.click(
			screen.getByRole("button", { name: "Import to NVMe deep dive" }),
		);

		expect(calls).toHaveBeenCalledWith(
			"start_ingest",
			expect.objectContaining({
				volume: "/Volumes/SONY",
				projectSlug: "nvme-deep-dive-2026-07-20",
				selectedPaths: ["PRIVATE/M4ROOT/CLIP/C0002.MP4"],
			}),
		);
		// The sheet swaps to the copy-progress panel for the started job.
		expect(
			await screen.findByText("Copying 1 clip → NVMe deep dive"),
		).toBeInTheDocument();
	});
});
