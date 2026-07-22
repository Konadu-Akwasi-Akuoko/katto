import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Dashboard } from "@/features/dashboard/dashboard";
import type { DriveStatus } from "@/lib/ipc/drive";
import type { Event } from "@/lib/ipc/events";
import type { Job } from "@/lib/ipc/jobs";
import { eventsFixture } from "@/test/fixtures/events";
import { jobsFixture } from "@/test/fixtures/jobs";

function renderDashboard({
	events = eventsFixture,
	jobs = jobsFixture,
	drive = { mounted: true, path: "/Volumes/Studio", free_gb: 412 },
}: {
	events?: Event[];
	jobs?: Job[];
	drive?: DriveStatus;
} = {}) {
	mockIPC((cmd) => {
		if (cmd === "list_events") return events;
		if (cmd === "list_jobs") return jobs;
		if (cmd === "get_drive_status") return drive;
		if (cmd === "subscribe_job_progress") return null;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return render(
		<QueryClientProvider client={client}>
			<Dashboard />
		</QueryClientProvider>,
	);
}

describe("Dashboard", () => {
	it("renders feed lines from events", async () => {
		renderDashboard();
		expect(await screen.findByText("Smoke test finished")).toBeInTheDocument();
		expect(screen.getByText("katto started")).toBeInTheDocument();
	});

	it("renders active jobs with a state chip", async () => {
		renderDashboard();
		expect(await screen.findByText("Smoke test")).toBeInTheDocument();
		expect(screen.getByText("running")).toBeInTheDocument();
	});

	it("shows the drive path and free space when mounted", async () => {
		renderDashboard();
		expect(await screen.findByText("/Volumes/Studio")).toBeInTheDocument();
		expect(screen.getByText("412 GB free")).toBeInTheDocument();
		expect(screen.getByText("mounted")).toBeInTheDocument();
	});

	it("shows plain empty states with no data", async () => {
		renderDashboard({
			events: [],
			jobs: [],
			drive: { mounted: true, path: null, free_gb: null },
		});
		expect(
			await screen.findByText(
				"Quiet so far. Everything katto does lands here — ingests, cuts, exports, downloads.",
			),
		).toBeInTheDocument();
		expect(
			screen.getByText(
				"Nothing running. Background work shows up here the moment it starts.",
			),
		).toBeInTheDocument();
		expect(screen.getByText("No studio root configured.")).toBeInTheDocument();
	});
});
