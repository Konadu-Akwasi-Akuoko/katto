import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { SurfaceSwitcher } from "@/components/layout/surface-switcher";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
	useUiStore.setState({ surface: "dashboard", switcherOpen: false });
});

describe("SurfaceSwitcher", () => {
	// the accessible name must contain the visible label, not replace it:
	// "click Browser" via voice control has to hit this control
	it("names the surface you are on", () => {
		useUiStore.setState({ surface: "browser" });
		render(<SurfaceSwitcher />);
		expect(screen.getByRole("button", { name: /Browser/ })).toHaveTextContent(
			"Browser",
		);
	});

	it("filters surfaces and navigates to the one picked", async () => {
		const user = userEvent.setup();
		render(<SurfaceSwitcher />);
		await user.click(screen.getByRole("button", { name: /Dashboard/ }));
		await user.type(await screen.findByPlaceholderText("Go to…"), "plan");

		expect(screen.queryByRole("option", { name: /Projects/ })).toBeNull();
		await user.click(await screen.findByRole("option", { name: /Planner/ }));

		expect(useUiStore.getState().surface).toBe("planner");
		expect(useUiStore.getState().switcherOpen).toBe(false);
	});

	// The popover hangs over the browser's content rect, so browser-surface reads
	// this to hide the native webview underneath it.
	it("publishes its open state to the ui store", async () => {
		const user = userEvent.setup();
		render(<SurfaceSwitcher />);
		await user.click(screen.getByRole("button", { name: /Dashboard/ }));
		expect(useUiStore.getState().switcherOpen).toBe(true);
	});

	it("gives the filter field a name of its own", async () => {
		const user = userEvent.setup();
		render(<SurfaceSwitcher />);
		await user.click(screen.getByRole("button", { name: /Dashboard/ }));
		expect(
			await screen.findByRole("combobox", { name: "Go to a surface" }),
		).toBeInTheDocument();
	});
});
