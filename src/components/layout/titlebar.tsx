import {
	MagnifyingGlassIcon,
	MoonIcon,
	PlusIcon,
	SidebarSimpleIcon,
	SunIcon,
} from "@phosphor-icons/react";
import type { ReactNode } from "react";
import { SurfaceSwitcher } from "@/components/layout/surface-switcher";
import { Button } from "@/components/ui/button";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { useUiStore } from "@/stores/ui";

export function Titlebar({
	dark,
	onToggleTheme,
	dock,
}: {
	dark: boolean;
	onToggleTheme: () => void;
	/**
	 * Compact Claude dock indicator, shown only while the sidebar is collapsed —
	 * the sidebar owns it otherwise, and two live copies would double the badge.
	 */
	dock?: ReactNode;
}) {
	const sidebarCollapsed = useUiStore((s) => s.sidebarCollapsed);
	return (
		<header className="flex items-center gap-3 border-b px-4 py-2.5 select-none">
			<span className="font-serif text-lg leading-none font-semibold">
				katto
			</span>
			<span className="text-xs text-fg-faint">alps-day-3</span>
			<Tooltip>
				<TooltipTrigger asChild>
					<Button
						variant="ghost"
						size="icon"
						aria-label="Toggle sidebar"
						onClick={() => useUiStore.getState().toggleSidebar()}
					>
						<SidebarSimpleIcon className="size-4" />
					</Button>
				</TooltipTrigger>
				<TooltipContent>Toggle sidebar</TooltipContent>
			</Tooltip>
			<SurfaceSwitcher />
			{sidebarCollapsed && dock}
			<div className="ml-auto flex items-center gap-2">
				<Button
					variant="outline"
					onClick={() => useUiStore.getState().setPaletteOpen(true)}
					className="hidden h-9 w-64 justify-start gap-2 px-3 font-normal text-fg-faint sm:flex"
				>
					<MagnifyingGlassIcon className="size-4" />
					Search or press ⌘K
				</Button>
				<Tooltip>
					<TooltipTrigger asChild>
						<Button variant="ghost" size="icon" onClick={onToggleTheme}>
							{dark ? (
								<SunIcon className="size-4" />
							) : (
								<MoonIcon className="size-4" />
							)}
						</Button>
					</TooltipTrigger>
					<TooltipContent>Toggle theme</TooltipContent>
				</Tooltip>
				<Button className="gap-1.5">
					<PlusIcon className="size-4" />
					New project
				</Button>
			</div>
		</header>
	);
}
