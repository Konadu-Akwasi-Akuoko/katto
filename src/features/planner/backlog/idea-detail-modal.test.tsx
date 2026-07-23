import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { IdeaDetailModal } from "@/features/planner/backlog/idea-detail-modal";
import { curatedFixture } from "@/test/fixtures/ideas";

afterEach(() => clearMocks());

function renderModal() {
	const idea = curatedFixture[0];
	if (idea === undefined) throw new Error("curatedFixture is empty");
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
		if (cmd === "update_idea") return { ...idea, ...(payload as object) };
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	const onClose = vi.fn();
	render(
		<QueryClientProvider client={client}>
			<IdeaDetailModal idea={idea} onClose={onClose} />
		</QueryClientProvider>,
	);
	return { calls, onClose, idea };
}

describe("IdeaDetailModal", () => {
	it("loads the idea and disables Save until a field changes", async () => {
		renderModal();
		expect(screen.getByLabelText("Name")).toHaveValue("SSD endurance myths");
		expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();

		await userEvent.type(screen.getByLabelText("Name"), "!");
		expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
	});

	it("Cancel closes the modal", async () => {
		const { onClose } = renderModal();
		await userEvent.type(screen.getByLabelText("Name"), "!");
		await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
		expect(onClose).toHaveBeenCalled();
	});

	it("saves the edited fields through update_idea", async () => {
		const { calls } = renderModal();
		await userEvent.clear(screen.getByLabelText("Name"));
		await userEvent.type(screen.getByLabelText("Name"), "New title");
		await userEvent.click(screen.getByRole("button", { name: "Save" }));
		await waitFor(() =>
			expect(calls).toHaveBeenCalledWith(
				"update_idea",
				expect.objectContaining({
					id: "idea-ai",
					patch: expect.objectContaining({
						title: "New title",
						lean: "strong",
					}),
				}),
			),
		);
	});
});
