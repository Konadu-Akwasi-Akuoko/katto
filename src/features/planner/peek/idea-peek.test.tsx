import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { IdeaPeek } from "@/features/planner/peek/idea-peek";
import { useUiStore } from "@/stores/ui";
import { backlogFixture } from "@/test/fixtures/ideas";

afterEach(() => {
	clearMocks();
	useUiStore.setState({ openIdeaId: null, surface: "planner" });
});

function renderPeek() {
	const idea = backlogFixture[0];
	if (idea === undefined) throw new Error("fixture is empty");
	mockIPC((cmd) => {
		if (cmd === "get_idea") return idea;
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<IdeaPeek />
		</QueryClientProvider>,
	);
	return idea;
}

describe("IdeaPeek", () => {
	it("renders nothing until an idea id is set", () => {
		renderPeek();
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});

	it("fetches and shows the idea when opened by id", async () => {
		const idea = renderPeek();
		useUiStore.setState({ openIdeaId: idea.id });
		expect(await screen.findByRole("dialog")).toBeInTheDocument();
		expect(screen.getByLabelText("Name")).toHaveValue(idea.title);
	});
});
