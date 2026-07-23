import { XIcon } from "@phosphor-icons/react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/**
 * A native date field that reads as genuinely empty when unset and clears back
 * to null on hover. WebKit paints today's date over an empty `type="date"` and
 * offers no clear affordance; this wraps both gaps. `value`/`onValueChange` model
 * the date as a nullable `YYYY-MM-DD` string. The native calendar indicator is
 * hidden in `main.css`; clicking the field opens the picker via `showPicker()`.
 */
function DateInput({
	value,
	onValueChange,
	label,
	className,
	disabled,
	...props
}: {
	value: string | null;
	onValueChange: (value: string | null) => void;
	label?: string;
} & Omit<React.ComponentProps<"input">, "value" | "onChange" | "type">) {
	const [focused, setFocused] = useState(false);
	const empty = value === null || value === "";
	// While focused the native segments are being typed, so let WebKit render
	// them; the placeholder only stands in for the resting, unset state.
	const showPlaceholder = empty && !focused;

	return (
		<div className="group relative w-40">
			<Input
				type="date"
				data-empty={showPlaceholder}
				className={cn("w-40 pr-8 font-mono tabular-nums", className)}
				value={value ?? ""}
				disabled={disabled}
				onFocus={() => setFocused(true)}
				onBlur={() => setFocused(false)}
				onClick={(event) => event.currentTarget.showPicker?.()}
				onChange={(event) => onValueChange(event.target.value || null)}
				{...props}
			/>
			{/* WebKit paints today's date over an empty date input; main.css hides
			    that ghost so this placeholder reads as genuinely unset. */}
			{showPlaceholder && (
				<span
					aria-hidden
					className="pointer-events-none absolute inset-y-0 left-3 flex items-center font-mono tabular-nums text-fg-faint"
				>
					—/—/—
				</span>
			)}
			{!empty && !disabled && (
				<Button
					type="button"
					variant="ghost"
					size="icon-xs"
					aria-label={label ? `Clear ${label} date` : "Clear date"}
					// Keep focus (and the picker) off the input when clearing.
					onMouseDown={(event) => event.preventDefault()}
					onClick={() => onValueChange(null)}
					className="absolute top-1/2 right-1.5 -translate-y-1/2 text-fg-faint opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
				>
					<XIcon className="size-4" />
				</Button>
			)}
		</div>
	);
}

export { DateInput };
