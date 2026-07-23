import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { CalendarView } from "@/features/planner/calendar/calendar-view";
import { useUiStore } from "@/stores/ui";

afterEach(() => {
	clearMocks();
	useUiStore.setState({ peekSlug: null, openIdeaId: null });
});

function renderCalendar() {
	mockIPC((cmd, payload) => {
		if (cmd === "list_calendar") {
			const { from } = payload as { from: string; to: string };
			return [
				{
					kind: "shoot",
					project_slug: "a",
					title: "Shoot A",
					date: from,
					note: null,
				},
				{
					kind: "publish",
					project_slug: "b",
					title: "Publish B",
					date: from,
					note: null,
				},
				{ kind: "backlog", idea_id: "i1", title: "Idea One", date: from },
			];
		}
		if (cmd === "list_projects") return [];
		throw new Error(`unexpected command: ${cmd}`);
	});
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	render(
		<QueryClientProvider client={client}>
			<CalendarView />
		</QueryClientProvider>,
	);
}

describe("CalendarView", () => {
	it("renders markers of each category", async () => {
		renderCalendar();
		expect(await screen.findByText("Shoot A")).toBeInTheDocument();
		expect(screen.getByText("Publish B")).toBeInTheDocument();
		expect(screen.getByText("Idea One")).toBeInTheDocument();
	});

	it("hides a category when its legend toggle is turned off", async () => {
		const user = userEvent.setup();
		renderCalendar();
		await screen.findByText("Publish B");

		await user.click(screen.getByRole("button", { name: "Publish" }));

		expect(screen.queryByText("Publish B")).not.toBeInTheDocument();
		expect(screen.getByText("Shoot A")).toBeInTheDocument();
	});

	it("opens the project peek from a shoot marker", async () => {
		const user = userEvent.setup();
		renderCalendar();
		await user.click(await screen.findByText("Shoot A"));
		expect(useUiStore.getState().peekSlug).toBe("a");
	});

	it("opens the idea modal from a backlog marker", async () => {
		const user = userEvent.setup();
		renderCalendar();
		await user.click(await screen.findByText("Idea One"));
		expect(useUiStore.getState().openIdeaId).toBe("i1");
	});
});
