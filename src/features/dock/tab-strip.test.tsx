import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/lib/ipc/bindings.gen";
import { TabStrip } from "./tab-strip";

function makeSession(
	id: string,
	label: string,
	state: SessionInfo["state"],
): SessionInfo {
	return {
		id,
		label,
		cwd: "/x",
		started_at: "2026-07-22 08:00:00",
		idle_since_secs: null,
		state,
	};
}

describe("TabStrip", () => {
	it("renders one tab per session, marks the active one, and shows notes", () => {
		const sessions = [
			makeSession("a", "ideas: nightly", { kind: "running" }),
			makeSession("b", "cut plan: intro.mp4", {
				kind: "closed",
				reason: "idle_reaped",
			}),
			makeSession("c", "vfx: glitch", {
				kind: "failed",
				error: "exited with status 3",
			}),
		];
		render(
			<TabStrip
				sessions={sessions}
				activeId="a"
				onSelect={() => {}}
				onClose={() => {}}
			/>,
		);
		const tabs = screen.getAllByRole("tab");
		expect(tabs).toHaveLength(3);
		expect(screen.getByRole("tab", { name: /ideas: nightly/ })).toHaveAttribute(
			"aria-selected",
			"true",
		);
		expect(
			screen.getByRole("tab", { name: /cut plan: intro.mp4/ }),
		).toHaveAttribute("aria-selected", "false");
		expect(screen.getByText("closed after idle")).toBeInTheDocument();
		expect(screen.getByText("exited with status 3")).toBeInTheDocument();
	});

	it("selects on tab click and closes on the close control", async () => {
		const user = userEvent.setup();
		const onSelect = vi.fn();
		const onClose = vi.fn();
		const sessions = [
			makeSession("a", "ideas: nightly", { kind: "idle" }),
			makeSession("b", "vfx: glitch", { kind: "running" }),
		];
		render(
			<TabStrip
				sessions={sessions}
				activeId="a"
				onSelect={onSelect}
				onClose={onClose}
			/>,
		);
		await user.click(screen.getByRole("tab", { name: /vfx: glitch/ }));
		expect(onSelect).toHaveBeenCalledWith("b");
		await user.click(
			screen.getByRole("button", { name: "close ideas: nightly" }),
		);
		expect(onClose).toHaveBeenCalledWith("a");
	});
});
