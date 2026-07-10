import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { CaptureForm } from "@/features/capture/capture-form";

function renderCapture() {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "capture_submit") return null;
		if (cmd === "plugin:window|close") return null;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { mutations: { networkMode: "always" } },
	});
	render(
		<QueryClientProvider client={client}>
			<CaptureForm />
		</QueryClientProvider>,
	);
	return { calls };
}

describe("CaptureForm", () => {
	it("submits the typed title and note on Enter", async () => {
		const user = userEvent.setup();
		const { calls } = renderCapture();

		const title = screen.getByRole("textbox", { name: /idea/i });
		await user.type(title, "Catch this thought");
		await user.type(screen.getByRole("textbox", { name: /note/i }), "later");
		await user.keyboard("{Enter}");

		expect(calls).toHaveBeenCalledWith("capture_submit", {
			title: "Catch this thought",
			note: "later",
			kind: null,
		});
	});

	it("does not submit an empty title", async () => {
		const user = userEvent.setup();
		const { calls } = renderCapture();

		await user.keyboard("{Enter}");

		expect(calls).not.toHaveBeenCalledWith("capture_submit", expect.anything());
	});
});
