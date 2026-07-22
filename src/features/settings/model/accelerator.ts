/** Mirrors `DEFAULT_CAPTURE_SHORTCUT` in src-tauri/src/capture.rs. A drift only
 * mis-shows the Reset affordance — resetting is idempotent either way. */
export const DEFAULT_CAPTURE_SHORTCUT = "alt+cmd+k";

/** KeyboardEvent.code → plugin key token, for codes the recorder accepts. */
const CODE_TOKENS: Record<string, string> = {
	Space: "space",
	ArrowUp: "up",
	ArrowDown: "down",
	ArrowLeft: "left",
	ArrowRight: "right",
	Comma: ",",
	Period: ".",
	Slash: "/",
	Semicolon: ";",
	Quote: "'",
	BracketLeft: "[",
	BracketRight: "]",
	Backslash: "\\",
	Backquote: "`",
	Minus: "-",
	Equal: "=",
};

const keyToken = (code: string): string | null => {
	const letter = /^Key([A-Z])$/.exec(code);
	if (letter?.[1]) return letter[1].toLowerCase();
	const digit = /^Digit([0-9])$/.exec(code);
	if (digit?.[1]) return digit[1];
	if (/^F([1-9]|1[0-2])$/.test(code)) return code.toLowerCase();
	return CODE_TOKENS[code] ?? null;
};

/**
 * Map a keydown to a plugin accelerator string ("alt+cmd+k"), or null when the
 * combo is unacceptable: no ⌘/⌃/⌥ (⇧ alone would shadow ordinary typing), a
 * modifier-only press, or a key the recorder rejects (Enter, Esc, editing keys).
 * Uses `code`, not `key`, so bindings are keyboard-layout-stable. Modifier
 * token order is fixed at ctrl+alt+shift+cmd so the default combo serializes
 * exactly as the stored "alt+cmd+k".
 */
export function acceleratorFromKeyboardEvent(
	ev: Pick<
		KeyboardEvent,
		"code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey"
	>,
): string | null {
	if (!(ev.metaKey || ev.ctrlKey || ev.altKey)) return null;
	const key = keyToken(ev.code);
	if (key === null) return null;
	const mods = [
		ev.ctrlKey ? "ctrl" : null,
		ev.altKey ? "alt" : null,
		ev.shiftKey ? "shift" : null,
		ev.metaKey ? "cmd" : null,
	].filter((m): m is string => m !== null);
	return [...mods, key].join("+");
}

const MOD_GLYPHS: Record<string, { glyph: string; order: number }> = {
	ctrl: { glyph: "⌃", order: 0 },
	control: { glyph: "⌃", order: 0 },
	alt: { glyph: "⌥", order: 1 },
	option: { glyph: "⌥", order: 1 },
	shift: { glyph: "⇧", order: 2 },
	cmd: { glyph: "⌘", order: 3 },
	command: { glyph: "⌘", order: 3 },
	super: { glyph: "⌘", order: 3 },
};

const KEY_GLYPHS: Record<string, string> = {
	space: "Space",
	up: "↑",
	down: "↓",
	left: "←",
	right: "→",
};

/**
 * Split an accelerator string into display chips: "alt+cmd+k" → ["⌥","⌘","K"].
 * Modifiers render in macOS-canonical ⌃⌥⇧⌘ order regardless of stored order.
 */
export function displayGlyphs(accel: string): string[] {
	const tokens = accel.split("+").map((t) => t.trim().toLowerCase());
	const mods: { glyph: string; order: number }[] = [];
	let key = "";
	for (const token of tokens) {
		const mod = MOD_GLYPHS[token];
		if (mod) mods.push(mod);
		else key = KEY_GLYPHS[token] ?? token.toUpperCase();
	}
	mods.sort((a, b) => a.order - b.order);
	return [...mods.map((m) => m.glyph), ...(key ? [key] : [])];
}
