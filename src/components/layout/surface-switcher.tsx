import { CaretUpDownIcon } from "@phosphor-icons/react";
import { SURFACE_NAV } from "@/components/layout/surfaces";
import { Button } from "@/components/ui/button";
import {
	Command,
	CommandEmpty,
	CommandGroup,
	CommandInput,
	CommandItem,
	CommandList,
} from "@/components/ui/command";
import {
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "@/components/ui/popover";
import type { Surface } from "@/stores/ui";
import { useUiStore } from "@/stores/ui";

/**
 * Navigation as a combobox: the trigger names where you are, the popover filters
 * everywhere you can go. Built from the Radix popover and cmdk command already
 * in `components/ui/` rather than a second component library — katto's overlays
 * all have to register with the browser surface so the native child webview can
 * be hidden underneath them, and a second portal/focus implementation would be
 * one more thing able to steal the address bar's focus.
 */
export function SurfaceSwitcher() {
	const surface = useUiStore((s) => s.surface);
	const setSurface = useUiStore((s) => s.setSurface);
	const open = useUiStore((s) => s.switcherOpen);
	const setOpen = useUiStore((s) => s.setSwitcherOpen);

	const current = SURFACE_NAV.find((s) => s.surface === surface);
	const CurrentIcon = current?.icon;

	function go(next: Surface) {
		setSurface(next);
		setOpen(false);
	}

	return (
		<Popover open={open} onOpenChange={setOpen}>
			<PopoverTrigger asChild>
				<Button variant="outline" size="sm" className="min-w-40 gap-2">
					{/* the accessible name has to *contain* the visible label, or
					    voice control on "click Browser" misses and screen readers
					    never hear which surface you are on */}
					<span className="sr-only">Switch surface, currently </span>
					{CurrentIcon !== undefined && (
						<CurrentIcon className="size-4 shrink-0 text-fg-muted" />
					)}
					<span className="truncate">{current?.label ?? "Go to"}</span>
					<CaretUpDownIcon className="ml-auto size-4 shrink-0 text-fg-faint" />
				</Button>
			</PopoverTrigger>
			<PopoverContent align="start" aria-label="Surfaces" className="w-56 p-0">
				<Command label="Go to a surface">
					<CommandInput placeholder="Go to…" />
					<CommandList>
						<CommandEmpty>Nothing by that name.</CommandEmpty>
						<CommandGroup>
							{SURFACE_NAV.map(({ surface: value, icon: Icon, label }) => (
								<CommandItem
									key={value}
									value={label}
									onSelect={() => go(value)}
								>
									<Icon className="size-4 text-fg-muted" />
									{label}
								</CommandItem>
							))}
						</CommandGroup>
					</CommandList>
				</Command>
			</PopoverContent>
		</Popover>
	);
}
