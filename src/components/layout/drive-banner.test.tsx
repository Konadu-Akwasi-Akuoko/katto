import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DriveBanner } from "@/components/layout/drive-banner";
import type { DriveStatus } from "@/lib/ipc/drive";

function renderWithStatus(status: DriveStatus) {
	mockIPC((cmd) => {
		if (cmd === "get_drive_status") return status;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return render(
		<QueryClientProvider client={client}>
			<DriveBanner />
		</QueryClientProvider>,
	);
}

describe("DriveBanner", () => {
	it("shows the expected path when the drive is disconnected", async () => {
		renderWithStatus({
			mounted: false,
			path: "/Volumes/Studio",
			free_gb: null,
		});
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"/Volumes/Studio",
		);
	});

	it("renders nothing while the drive is mounted", async () => {
		renderWithStatus({ mounted: true, path: "/Volumes/Studio", free_gb: 412 });
		expect(screen.queryByRole("alert")).not.toBeInTheDocument();
	});
});
