import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { settingsFixture } from "@/test/fixtures/settings";
import { ImportSection } from "./import-section";

const DEFAULT_PATH = "~/Projects/WebDev/hyper-frames/tools/studio/studio.db";

function renderSection(options: { failDryRun?: boolean } = {}) {
	const calls = vi.fn();
	mockIPC((cmd, args) => {
		calls(cmd, args);
		if (cmd === "import_studio_db") {
			if (options.failDryRun) {
				throw {
					kind: "import_failed",
					message: "couldn't read studio.db: not a database",
				};
			}
			const a = args as { path: string; dryRun: boolean };
			if (a.dryRun) {
				return {
					kind: "preview",
					report: {
						imported: 12,
						updated: 3,
						skipped: 40,
						warnings: ['skipped c3: unknown status "weird"'],
					},
				};
			}
			return { kind: "started", job_id: "job-1" };
		}
		return null;
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<ImportSection settings={settingsFixture} />
		</QueryClientProvider>,
	);
	return { calls };
}

describe("ImportSection", () => {
	it("prefills the default path and offers a dry run", () => {
		renderSection();
		expect(screen.getByRole("textbox", { name: "studio.db path" })).toHaveValue(
			DEFAULT_PATH,
		);
		expect(screen.getByRole("button", { name: "Dry run" })).toBeEnabled();
	});

	it("renders the dry-run counts and enables the import", async () => {
		const { calls } = renderSection();
		await userEvent.click(screen.getByRole("button", { name: "Dry run" }));
		expect(await screen.findByText("12")).toBeInTheDocument();
		expect(screen.getByText("3")).toBeInTheDocument();
		expect(screen.getByText("40")).toBeInTheDocument();
		expect(
			screen.getByText('skipped c3: unknown status "weird"'),
		).toBeInTheDocument();
		const importButton = screen.getByRole("button", {
			name: "Import 12 ideas",
		});
		expect(importButton).toBeEnabled();

		await userEvent.click(importButton);
		await waitFor(() => {
			expect(calls).toHaveBeenCalledWith("import_studio_db", {
				path: DEFAULT_PATH,
				dryRun: false,
			});
		});
	});

	it("renders import failures inline, not as a toast", async () => {
		renderSection({ failDryRun: true });
		await userEvent.click(screen.getByRole("button", { name: "Dry run" }));
		expect(
			await screen.findByText("couldn't read studio.db: not a database"),
		).toBeInTheDocument();
	});
});
