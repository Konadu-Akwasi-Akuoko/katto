import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearCommands, registerCommand } from "@/features/palette/registry";
import { Palette } from "@/features/palette/palette";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
	useUiStore.setState({ surface: "dashboard", paletteOpen: false });
});

afterEach(() => {
	clearCommands();
});

describe("Palette", () => {
	it("opens on meta+k and runs the selected command", async () => {
		const run = vi.fn();
		registerCommand({ id: "t", title: "Do the thing", keywords: ["thing"], group: "Test", run });
		const user = userEvent.setup();
		render(<Palette />);

		await user.keyboard("{Meta>}k{/Meta}");
		const item = await screen.findByText("Do the thing");
		await user.click(item);

		expect(run).toHaveBeenCalledOnce();
		await waitFor(() => expect(useUiStore.getState().paletteOpen).toBe(false));
	});

	it("closes on a second meta+k press", async () => {
		registerCommand({ id: "t", title: "Do the thing", keywords: [], group: "Test", run: vi.fn() });
		const user = userEvent.setup();
		render(<Palette />);

		await user.keyboard("{Meta>}k{/Meta}");
		expect(await screen.findByPlaceholderText("Type a command…")).toBeInTheDocument();
		await user.keyboard("{Meta>}k{/Meta}");
		await waitFor(() => expect(useUiStore.getState().paletteOpen).toBe(false));
	});

	it("shows the empty state when nothing matches", async () => {
		registerCommand({ id: "t", title: "Do the thing", keywords: [], group: "Test", run: vi.fn() });
		const user = userEvent.setup();
		render(<Palette />);

		useUiStore.getState().setPaletteOpen(true);
		await user.type(await screen.findByPlaceholderText("Type a command…"), "zzz");
		expect(await screen.findByText("No matching command.")).toBeInTheDocument();
	});
});
