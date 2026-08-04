import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { Sidebar } from "@/components/layout/sidebar";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
	useUiStore.setState({ surface: "dashboard", sidebarCollapsed: false });
});

describe("Sidebar", () => {
	it("marks the surface you are on", () => {
		useUiStore.setState({ surface: "planner" });
		render(<Sidebar />);
		expect(screen.getByRole("button", { name: "Planner" })).toHaveAttribute(
			"aria-current",
			"page",
		);
	});

	it("navigates on click", async () => {
		const user = userEvent.setup();
		render(<Sidebar />);
		await user.click(screen.getByRole("button", { name: "Projects" }));
		expect(useUiStore.getState().surface).toBe("projects");
	});

	// collapsed means gone, not narrow — an icon rail would be a second,
	// redundant surface indicator still charging rent on the page width
	it("renders nothing when collapsed", () => {
		useUiStore.setState({ sidebarCollapsed: true });
		const { container } = render(<Sidebar dock={<span>dock</span>} />);
		expect(container).toBeEmptyDOMElement();
	});

	it("keeps the dock alongside the studio surfaces when expanded", () => {
		render(<Sidebar dock={<span>dock</span>} />);
		expect(screen.getByText("dock")).toBeInTheDocument();
	});
});
