import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CalendarView } from "@/features/planner/calendar/calendar-view";
import { useUiStore } from "@/stores/ui";
import { project } from "@/test/fixtures/projects";

afterEach(() => {
	clearMocks();
	useUiStore.setState({ peekSlug: null, openIdeaId: null });
});

function renderCalendar({ mounted = true }: { mounted?: boolean } = {}) {
	const calls = vi.fn();
	mockIPC((cmd, payload) => {
		calls(cmd, payload);
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
		if (cmd === "list_projects")
			return [project({ slug: "a", title: "Shoot A" })];
		if (cmd === "get_drive_status")
			return {
				mounted,
				path: mounted ? "/Volumes/Studio" : null,
				free_gb: null,
			};
		if (cmd === "upsert_schedule_entry")
			return {
				id: 1,
				project_slug: "a",
				kind: "shoot",
				date: "2026-07-10",
				note: null,
			};
		if (cmd === "delete_schedule_entry") return null;
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
	return { calls };
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

	it("opens the idea modal from a backlog marker", async () => {
		const user = userEvent.setup();
		renderCalendar();
		await user.click(await screen.findByText("Idea One"));
		expect(useUiStore.getState().openIdeaId).toBe("i1");
	});

	it("saves a moved pin through upsert_schedule_entry from a shoot marker", async () => {
		const user = userEvent.setup();
		const { calls } = renderCalendar();
		await user.click(await screen.findByText("Shoot A"));
		await user.click(await screen.findByRole("button", { name: "Save" }));
		expect(calls).toHaveBeenCalledWith(
			"upsert_schedule_entry",
			expect.objectContaining({ projectSlug: "a", kind: "shoot" }),
		);
	});

	it("clears a pin through delete_schedule_entry", async () => {
		const user = userEvent.setup();
		const { calls } = renderCalendar();
		await user.click(await screen.findByText("Shoot A"));
		await user.click(await screen.findByRole("button", { name: "Clear" }));
		expect(calls).toHaveBeenCalledWith("delete_schedule_entry", {
			projectSlug: "a",
			kind: "shoot",
		});
	});

	it("disables pin editing on an unmounted studio root", async () => {
		const user = userEvent.setup();
		renderCalendar({ mounted: false });
		await user.click(await screen.findByText("Shoot A"));
		expect(await screen.findByRole("button", { name: "Save" })).toBeDisabled();
		expect(screen.getByRole("button", { name: "Clear" })).toBeDisabled();
	});
});
