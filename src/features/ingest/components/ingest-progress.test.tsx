import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { IngestProgress } from "@/features/ingest/components/ingest-progress";

const failedJob = {
	id: "job-1",
	kind: "ingest",
	label: "Import 3 clips",
	status: "failed",
	progress: 0.34,
	payload_json: null,
	error: "cannot stat source /Volumes/SONY/CLIP/C0002.MP4",
	started_at: "2026-07-22T10:00:00Z",
	finished_at: "2026-07-22T10:01:00Z",
};

const failedEvent = {
	id: 7,
	ts: "2026-07-22T10:01:00Z",
	kind: "ingest_failed",
	project_slug: "nvme-deep-dive-2026-07-20",
	payload_json: JSON.stringify({
		job_id: "job-1",
		remaining: ["CLIP/C0002.MP4", "CLIP/C0003.MP4"],
	}),
};

function renderPanel(onRetry?: (remaining: string[]) => void) {
	mockIPC((cmd) => {
		if (cmd === "subscribe_job_progress") return null;
		if (cmd === "list_jobs") return [failedJob];
		if (cmd === "list_events") return [failedEvent];
		if (typeof cmd === "string" && cmd.startsWith("plugin:event|")) return 1;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<IngestProgress
				jobId="job-1"
				volume="/Volumes/SONY"
				projectTitle="NVMe deep dive"
				clipCount={3}
				onRetry={onRetry}
			/>
		</QueryClientProvider>,
	);
}

describe("IngestProgress", () => {
	it("shows the failed job's error and offers the retry remainder", async () => {
		const onRetry = vi.fn();
		renderPanel(onRetry);
		const user = userEvent.setup();

		expect(
			await screen.findByText("Import into NVMe deep dive failed"),
		).toBeInTheDocument();
		expect(
			screen.getByText("cannot stat source /Volumes/SONY/CLIP/C0002.MP4"),
		).toBeInTheDocument();

		await user.click(
			await screen.findByRole("button", { name: "Retry remaining 2 clips" }),
		);
		expect(onRetry).toHaveBeenCalledWith(["CLIP/C0002.MP4", "CLIP/C0003.MP4"]);
	});

	it("hides the retry affordance when no retry callback is available", async () => {
		renderPanel(undefined);
		expect(
			await screen.findByText("Import into NVMe deep dive failed"),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /Retry remaining/ }),
		).not.toBeInTheDocument();
	});
});
