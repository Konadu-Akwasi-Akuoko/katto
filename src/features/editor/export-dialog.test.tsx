import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ExportDialog } from "@/features/editor/export-dialog";
import type { ExportPreview, ExportResult } from "@/lib/ipc/editor";

const noop = () => {};

function mockCommands(
	preview: ExportPreview,
	result?: ExportResult,
	resolveAvailable = false,
): ReturnType<typeof vi.fn> {
	const exportSpy = vi.fn();
	mockIPC((cmd, args) => {
		if (cmd === "preview_export") return preview;
		if (cmd === "resolve_available") return resolveAvailable;
		if (cmd === "export_timeline") {
			const a = args as {
				bundlePath: string;
				nleTarget: string;
				openAfter: boolean;
			};
			exportSpy(a.bundlePath, a.nleTarget, a.openAfter);
			if (result === undefined) throw new Error("no export result mocked");
			return result;
		}
		throw new Error(`unmocked command ${cmd}`);
	});
	return exportSpy;
}

const doneResult: ExportResult = {
	fcpxml_path: "/t/demo-v1.fcpxml",
	srt_path: "/t/demo-v1.srt",
	vtt_path: "/t/demo-v1.vtt",
	version: 1,
	opened_in_nle: false,
	revealed: false,
};

describe("ExportDialog", () => {
	it("forces an NLE pick on first export and previews the exact filename", async () => {
		mockCommands({ slug: "nvme-deep-dive", version: 3, default_nle: null });
		render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
		expect(
			await screen.findByText("nvme-deep-dive-v3.fcpxml"),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Export" })).toBeDisabled();
		fireEvent.click(screen.getByRole("radio", { name: /Final Cut/ }));
		expect(screen.getByRole("button", { name: "Export" })).toBeEnabled();
	});

	it("preselects the sticky default and exports through it", async () => {
		const exportSpy = mockCommands(
			{ slug: "demo", version: 1, default_nle: "final_cut" },
			doneResult,
		);
		render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
		expect(
			await screen.findByRole("radio", { name: /Final Cut/ }),
		).toBeChecked();
		fireEvent.click(screen.getByRole("button", { name: "Export" }));
		await waitFor(() =>
			expect(exportSpy).toHaveBeenCalledWith("/b", "final_cut", false),
		);
	});

	it("offers Open in Final Cut after a successful export", async () => {
		mockCommands(
			{ slug: "demo", version: 1, default_nle: "final_cut" },
			doneResult,
		);
		render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
		fireEvent.click(await screen.findByRole("button", { name: "Export" }));
		expect(
			await screen.findByRole("button", { name: "Open in Final Cut" }),
		).toBeInTheDocument();
		expect(screen.getByText("/t/demo-v1.srt")).toBeInTheDocument();
	});

	it("offers Open in Resolve only when Resolve is installed", async () => {
		mockCommands(
			{ slug: "demo", version: 1, default_nle: "resolve" },
			doneResult,
			true,
		);
		render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
		fireEvent.click(await screen.findByRole("button", { name: "Export" }));
		expect(
			await screen.findByRole("button", { name: "Open in Resolve" }),
		).toBeInTheDocument();
	});

	it("falls back to Reveal in Finder when Resolve is absent", async () => {
		mockCommands(
			{ slug: "demo", version: 1, default_nle: "resolve" },
			doneResult,
			false,
		);
		render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
		fireEvent.click(await screen.findByRole("button", { name: "Export" }));
		expect(
			await screen.findByRole("button", { name: "Reveal in Finder" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Open in Resolve" }),
		).not.toBeInTheDocument();
	});

	it("renders engine invariant errors inline and stays open", async () => {
		mockIPC((cmd) => {
			if (cmd === "preview_export") {
				return { slug: "demo", version: 1, default_nle: "final_cut" };
			}
			if (cmd === "export_timeline") {
				throw { kind: "engine", message: "fcpxml invariant: clip 0 off grid" };
			}
			throw new Error(`unmocked command ${cmd}`);
		});
		render(<ExportDialog bundlePath="/b" onClose={noop} onExported={noop} />);
		fireEvent.click(await screen.findByRole("button", { name: "Export" }));
		expect(
			await screen.findByText(/fcpxml invariant: clip 0 off grid/),
		).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Export" })).toBeInTheDocument();
	});
});
