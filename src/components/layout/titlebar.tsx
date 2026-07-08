import {
	MagnifyingGlassIcon,
	MoonIcon,
	PlusIcon,
	SunIcon,
} from "@phosphor-icons/react";
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
}: {
	dark: boolean;
	onToggleTheme: () => void;
}) {
	return (
		<header className="flex items-center gap-3 border-b px-4 py-2.5 select-none">
			<span className="font-serif text-lg leading-none font-semibold">
				katto
			</span>
			<span className="text-xs text-fg-faint">alps-day-3</span>
			<div className="ml-auto flex items-center gap-2">
				<button
					type="button"
					onClick={() => useUiStore.getState().setPaletteOpen(true)}
					className="hidden h-9 w-64 items-center gap-2 rounded-md border bg-surface px-3 text-sm text-fg-faint sm:flex"
				>
					<MagnifyingGlassIcon className="size-4" />
					Search or press ⌘K
				</button>
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
