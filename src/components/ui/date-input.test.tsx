import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DateInput } from "@/components/ui/date-input";

describe("DateInput", () => {
	it("shows the placeholder and no clear button when unset", () => {
		render(<DateInput value={null} onValueChange={vi.fn()} />);
		expect(screen.getByText("—/—/—")).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: /clear/i }),
		).not.toBeInTheDocument();
	});

	it("clears a set date back to null", async () => {
		const user = userEvent.setup();
		const onValueChange = vi.fn();
		render(
			<DateInput
				value="2026-07-31"
				label="Publish"
				onValueChange={onValueChange}
			/>,
		);
		expect(screen.queryByText("—/—/—")).not.toBeInTheDocument();
		await user.click(
			screen.getByRole("button", { name: "Clear Publish date" }),
		);
		expect(onValueChange).toHaveBeenCalledWith(null);
	});
});
