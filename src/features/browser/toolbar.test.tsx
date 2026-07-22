import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TabSnapshot } from "@/lib/ipc/browser";
import { Toolbar } from "./toolbar";

function tab(url: string): TabSnapshot {
	return {
		id: 1,
		title: url,
		url,
		can_go_back: false,
		can_go_forward: false,
	};
}

describe("Toolbar", () => {
	it("follows navigation while the address bar is not being edited", () => {
		const { rerender } = render(
			<Toolbar
				activeTab={tab("https://a.test/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		rerender(
			<Toolbar
				activeTab={tab("https://b.test/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		expect(screen.getByRole("textbox", { name: "Address" })).toHaveValue(
			"b.test",
		);
	});

	it("keeps the draft when a navigation lands mid-edit", () => {
		const { rerender } = render(
			<Toolbar
				activeTab={tab("https://a.test/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		const input = screen.getByRole("textbox", { name: "Address" });
		fireEvent.focus(input);
		fireEvent.change(input, { target: { value: "example.co" } });
		// a client-side redirect refetches browser_state mid-typing
		rerender(
			<Toolbar
				activeTab={tab("https://b.test/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		expect(input).toHaveValue("example.co");
		// blurring without submitting resyncs to the page
		fireEvent.blur(input);
		rerender(
			<Toolbar
				activeTab={tab("https://c.test/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		expect(input).toHaveValue("c.test");
	});
});
