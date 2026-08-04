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

	it("searches when the input is not an address", () => {
		const onNavigate = vi.fn();
		render(
			<Toolbar
				activeTab={tab("https://a.test/")}
				onNavigate={onNavigate}
				onGo={vi.fn()}
			/>,
		);
		const input = screen.getByRole("textbox", { name: "Address" });
		fireEvent.focus(input);
		fireEvent.change(input, { target: { value: "dust particles" } });
		fireEvent.keyDown(input, { key: "Enter" });
		expect(onNavigate).toHaveBeenCalledWith(
			"https://www.google.com/search?q=dust%20particles",
		);
	});

	// The reported bug: typing a search and pressing Enter left the field showing
	// the query for the life of the tab, because focus alone latched "editing"
	// and Enter never blurs.
	it("follows navigation after Enter while the input keeps focus", () => {
		const { rerender } = render(
			<Toolbar activeTab={null} onNavigate={vi.fn()} onGo={vi.fn()} />,
		);
		const input = screen.getByRole("textbox", { name: "Address" });
		fireEvent.focus(input);
		fireEvent.change(input, { target: { value: "youtube studio" } });
		fireEvent.keyDown(input, { key: "Enter" });
		// no blur — the address bar keeps focus exactly as it does in the app
		rerender(
			<Toolbar
				activeTab={tab("https://studio.youtube.com/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		expect(input).toHaveValue("studio.youtube.com");
	});

	it("follows navigation while focused but unedited", () => {
		const { rerender } = render(
			<Toolbar
				activeTab={tab("https://a.test/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		const input = screen.getByRole("textbox", { name: "Address" });
		fireEvent.focus(input);
		rerender(
			<Toolbar
				activeTab={tab("https://b.test/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		expect(input).toHaveValue("b.test");
	});

	it("selects the whole address on the click that focuses it", () => {
		render(
			<Toolbar
				activeTab={tab("https://a.test/")}
				onNavigate={vi.fn()}
				onGo={vi.fn()}
			/>,
		);
		const input = screen.getByRole("textbox", {
			name: "Address",
		}) as HTMLInputElement;

		// WebKit sets the caret in mousedown's default action, so the handler
		// suppresses it and takes focus itself; onFocus then selects.
		expect(fireEvent.mouseDown(input)).toBe(false);
		expect(document.activeElement).toBe(input);
		expect(input.selectionStart).toBe(0);
		expect(input.selectionEnd).toBe(input.value.length);

		// a second click on the focused field is left alone, so the caret lands
		// where the user pressed
		expect(fireEvent.mouseDown(input)).toBe(true);
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
