import type { ReactNode } from "react";

/**
 * The desktop app shell. The window itself never scrolls: this is a fixed
 * 100dvh grid (titlebar row / banner row / body row), and the body splits into
 * a fixed sidebar and a single scrollable content pane. `min-h-0` on the grid
 * tracks is what lets the content pane actually scroll instead of growing the
 * frame. The banner wrapper keeps the row count stable whether or not a banner
 * renders — an empty `auto` row is 0px. The body is flex, not grid, because the
 * sidebar renders nothing when collapsed and a two-column grid would leave the
 * content pane stranded in the auto-sized first column.
 */
export function AppShell({
	titlebar,
	banner,
	sidebar,
	overlay,
	children,
}: {
	titlebar: ReactNode;
	banner?: ReactNode;
	sidebar: ReactNode;
	/** Absolutely-positioned layer over the content pane (the Claude dock). */
	overlay?: ReactNode;
	children: ReactNode;
}) {
	return (
		<div className="grid h-dvh grid-rows-[auto_auto_1fr] overflow-hidden">
			{titlebar}
			<div>{banner}</div>
			<div className="flex min-h-0">
				{sidebar}
				<div className="relative grid min-h-0 flex-1">
					<main
						data-scroll-root
						className="min-h-0 overflow-y-auto overscroll-contain"
					>
						{children}
					</main>
					{overlay}
				</div>
			</div>
		</div>
	);
}
