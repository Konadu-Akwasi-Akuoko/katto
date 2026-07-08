import { useEffect } from "react";
import { toast } from "sonner";
import {
	CommandDialog,
	CommandEmpty,
	CommandGroup,
	CommandInput,
	CommandItem,
	CommandList,
} from "@/components/ui/command";
import type { PaletteCommand } from "@/features/palette/registry";
import { listCommands } from "@/features/palette/registry";
import { useUiStore } from "@/stores/ui";

/** App-wide ⌘K palette. Entries come from the registry; the app registers its
 * commands at startup and features add theirs as they land. */
export function Palette() {
	const open = useUiStore((s) => s.paletteOpen);
	const setPaletteOpen = useUiStore((s) => s.setPaletteOpen);

	useEffect(() => {
		function onKeyDown(event: KeyboardEvent) {
			if (event.key === "k" && event.metaKey && !event.repeat) {
				event.preventDefault();
				useUiStore.getState().togglePalette();
			}
		}
		window.addEventListener("keydown", onKeyDown);
		return () => window.removeEventListener("keydown", onKeyDown);
	}, []);

	const groups = new Map<string, PaletteCommand[]>();
	for (const command of listCommands()) {
		const bucket = groups.get(command.group) ?? [];
		bucket.push(command);
		groups.set(command.group, bucket);
	}

	return (
		<CommandDialog
			open={open}
			onOpenChange={setPaletteOpen}
			showCloseButton={false}
		>
			<CommandInput placeholder="Type a command…" />
			<CommandList>
				<CommandEmpty>No matching command.</CommandEmpty>
				{[...groups.entries()].map(([heading, entries]) => (
					<CommandGroup key={heading} heading={heading}>
						{entries.map((command) => (
							<CommandItem
								key={command.id}
								value={`${command.title} ${command.keywords.join(" ")}`}
								onSelect={() => {
									setPaletteOpen(false);
									void Promise.resolve(command.run()).catch((error: unknown) => {
										const description =
											error instanceof Error ? error.message : String(error);
										toast.error("Command failed", { description });
									});
								}}
							>
								{command.title}
							</CommandItem>
						))}
					</CommandGroup>
				))}
			</CommandList>
		</CommandDialog>
	);
}
