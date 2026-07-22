import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockConvertFileSrc, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { toast } from "sonner";
import { describe, expect, it, vi } from "vitest";
import { ThumbnailsCard } from "./thumbnails-card";

vi.mock("sonner", () => ({
	toast: { info: vi.fn(), error: vi.fn(), success: vi.fn(), warning: vi.fn() },
}));

function renderCard(options: {
	latest: { slug: string; path: string; mtime_ms: number } | null;
	opened?: "photoshop" | "revealed_in_finder";
}) {
	const calls = vi.fn();
	mockConvertFileSrc("asset");
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "latest_thumbnail") return options.latest;
		if (cmd === "create_thumbnail")
			return {
				psd_path:
					"/studio/Projects/sprint-recap/thumbnails/sprint-recap-thumb-a.psd",
				opened: options.opened ?? "photoshop",
			};
		return null;
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<ThumbnailsCard slug="sprint-recap" />
		</QueryClientProvider>,
	);
	return { calls };
}

describe("ThumbnailsCard", () => {
	it("shows the empty state and scaffolds a landscape template", async () => {
		const { calls } = renderCard({ latest: null });
		expect(
			await screen.findByText(/No thumbnail yet/, { exact: false }),
		).toBeInTheDocument();

		await userEvent.click(
			screen.getByRole("button", { name: "New thumbnail" }),
		);
		expect(await screen.findByText("1280 × 720")).toBeInTheDocument();
		expect(screen.getByText("1080 × 1920")).toBeInTheDocument();

		await userEvent.click(screen.getByText("YouTube"));
		await waitFor(() => {
			expect(calls).toHaveBeenCalledWith("create_thumbnail", {
				slug: "sprint-recap",
				format: "landscape",
			});
		});
	});

	it("notes the Finder fallback when Photoshop is missing", async () => {
		renderCard({ latest: null, opened: "revealed_in_finder" });
		await userEvent.click(
			await screen.findByRole("button", { name: "New thumbnail" }),
		);
		await userEvent.click(await screen.findByText("YouTube"));
		await waitFor(() => {
			expect(toast.info).toHaveBeenCalledWith(
				"Photoshop not found — revealed in Finder",
				expect.anything(),
			);
		});
	});

	it("renders the newest PNG when one exists", async () => {
		renderCard({
			latest: {
				slug: "sprint-recap",
				path: "/studio/Projects/sprint-recap/thumbnails/final.png",
				mtime_ms: 1_753_000_000_000,
			},
		});
		const image = await screen.findByAltText(
			"Newest thumbnail for sprint-recap",
		);
		expect(image).toBeInTheDocument();
		expect(screen.getByText("final.png")).toBeInTheDocument();
	});
});
