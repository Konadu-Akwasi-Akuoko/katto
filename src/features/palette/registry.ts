/** One executable entry in the ⌘K palette. */
export type PaletteCommand = {
	id: string;
	title: string;
	keywords: string[];
	group: string;
	run: () => void | Promise<void>;
};

const registry = new Map<string, PaletteCommand>();

/** Register (or replace, keyed by id) a palette command. */
export function registerCommand(command: PaletteCommand): void {
	registry.set(command.id, command);
}

/** All registered commands, in first-registration order. */
export function listCommands(): PaletteCommand[] {
	return [...registry.values()];
}

/** Test hook: empty the registry. */
export function clearCommands(): void {
	registry.clear();
}
