import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render as rtlRender, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { project } from "@/test/fixtures/projects";
import { StartPage } from "./start-page";

/** The facts row reads the filing target and the project list over IPC. */
function render(ui: React.ReactElement) {
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return rtlRender(
		<QueryClientProvider client={client}>{ui}</QueryClientProvider>,
	);
}

beforeEach(() => {
	mockIPC((cmd) => {
		if (cmd === "active_asset_project") return "nvme-deep-dive-2026-07-08";
		if (cmd === "list_projects")
			return [
				project({
					slug: "nvme-deep-dive-2026-07-08",
					title: "NVMe deep dive",
				}),
			];
		return null;
	});
});

const TILE_NAMES = [
	"Envato Elements",
	"Dribbble",
	"Pinterest",
	"TestMyThumbnails",
	"YouTube Studio",
	"Unsplash",
	"Freesound",
	"Coolors",
];

describe("StartPage", () => {
	it("renders every tile", () => {
		render(<StartPage onNavigate={vi.fn()} />);
		for (const name of TILE_NAMES) {
			expect(screen.getByRole("button", { name })).toBeInTheDocument();
		}
	});

	it("navigates to the tile's url on click", () => {
		const onNavigate = vi.fn();
		render(<StartPage onNavigate={onNavigate} />);
		fireEvent.click(screen.getByRole("button", { name: "Freesound" }));
		expect(onNavigate).toHaveBeenCalledWith("https://freesound.org/");
	});

	it("searches free text entered in the field", () => {
		const onNavigate = vi.fn();
		render(<StartPage onNavigate={onNavigate} />);
		const field = screen.getByRole("textbox", {
			name: "Search or enter address",
		});
		fireEvent.change(field, { target: { value: "dust particles" } });
		fireEvent.keyDown(field, { key: "Enter" });
		expect(onNavigate).toHaveBeenCalledWith(
			"https://www.google.com/search?q=dust%20particles",
		);
	});

	it("navigates when the field holds an address", () => {
		const onNavigate = vi.fn();
		render(<StartPage onNavigate={onNavigate} />);
		const field = screen.getByRole("textbox", {
			name: "Search or enter address",
		});
		fireEvent.change(field, { target: { value: "example.com" } });
		fireEvent.keyDown(field, { key: "Enter" });
		expect(onNavigate).toHaveBeenCalledWith("https://example.com");
	});
});
