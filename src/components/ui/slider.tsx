import { Slider as SliderPrimitive } from "radix-ui";
import * as React from "react";

import { cn } from "@/lib/utils";

function Slider({
	className,
	defaultValue,
	value,
	min = 0,
	max = 100,
	...props
}: React.ComponentProps<typeof SliderPrimitive.Root>) {
	const values = React.useMemo(
		() =>
			Array.isArray(value)
				? value
				: Array.isArray(defaultValue)
					? defaultValue
					: [min, max],
		[value, defaultValue, min, max],
	);

	return (
		<SliderPrimitive.Root
			data-slot="slider"
			defaultValue={defaultValue}
			value={value}
			min={min}
			max={max}
			className={cn(
				"relative flex w-full touch-none select-none items-center data-[disabled]:opacity-45",
				className,
			)}
			{...props}
		>
			<SliderPrimitive.Track
				data-slot="slider-track"
				className="relative h-1 w-full grow overflow-hidden rounded-full bg-surface-2"
			>
				<SliderPrimitive.Range
					data-slot="slider-range"
					className="absolute h-full bg-ember"
				/>
			</SliderPrimitive.Track>
			{Array.from({ length: values.length }, (_, index) => (
				<SliderPrimitive.Thumb
					data-slot="slider-thumb"
					// biome-ignore lint/suspicious/noArrayIndexKey: thumbs are positional by definition
					key={index}
					className={cn(
						"block size-3 shrink-0 cursor-default rounded-full border border-border bg-fg",
						"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2",
						"disabled:opacity-45",
					)}
				/>
			))}
		</SliderPrimitive.Root>
	);
}

export { Slider };
