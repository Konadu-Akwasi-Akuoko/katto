import type * as React from "react";

import { cn } from "@/lib/utils";

/**
 * A quiet inline warning surface. Opts out of the baked-in grain because the
 * translucent fill would amplify the noise (design-system translucent-fill rule).
 */
function Callout({
	className,
	children,
	...props
}: React.ComponentProps<"div">) {
	return (
		<div
			data-slot="callout"
			style={{ backgroundImage: "none" }}
			className={cn(
				"flex items-start gap-2 rounded-[var(--r)] border border-warn/40 bg-warn/10 px-3 py-2 text-sm text-fg",
				className,
			)}
			{...props}
		>
			{children}
		</div>
	);
}

export { Callout };
