import { ArrowLeftIcon, ArrowRightIcon } from "@phosphor-icons/react";
import { useState } from "react";
import type { TabSnapshot } from "@/lib/ipc/browser";
import { displayUrl, toNavigable } from "./model/address";

/**
 * Back/forward + address bar. The input stays mono because it predominantly
 * shows URLs, which are machine data. Input that does not parse as an address
 * is searched (see `model/address.ts`).
 */
export function Toolbar({
	activeTab,
	onNavigate,
	onGo,
	children,
}: {
	activeTab: TabSnapshot | null;
	onNavigate: (url: string) => void;
	onGo: (delta: number) => void;
	children?: React.ReactNode;
}) {
	// The query owns what the field shows; `draft` exists only while the user is
	// actually typing (null otherwise) and simply takes precedence. There is no
	// mirrored copy to reconcile, so navigation can never be "lost" behind a
	// stale edit flag — the previous version latched on focus and, because Enter
	// never blurs, froze the field on the typed text for the life of the tab.
	// Editing means typing, not merely holding focus.
	const [draft, setDraft] = useState<string | null>(null);
	const currentUrl = activeTab?.url ?? "";
	const shown = draft ?? (currentUrl === "" ? "" : displayUrl(currentUrl));

	function submit() {
		const next = toNavigable(shown);
		if (next === null) return;
		setDraft(null);
		onNavigate(next);
	}

	return (
		<div className="flex h-10 shrink-0 items-center gap-1.5 border-b px-2">
			<button
				type="button"
				aria-label="Back"
				disabled={!activeTab?.can_go_back}
				onClick={() => onGo(-1)}
				className="flex size-7 items-center justify-center rounded-md text-fg-muted hover:bg-surface-2 hover:text-fg disabled:pointer-events-none disabled:opacity-45 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
			>
				<ArrowLeftIcon className="size-4" />
			</button>
			<button
				type="button"
				aria-label="Forward"
				disabled={!activeTab?.can_go_forward}
				onClick={() => onGo(1)}
				className="flex size-7 items-center justify-center rounded-md text-fg-muted hover:bg-surface-2 hover:text-fg disabled:pointer-events-none disabled:opacity-45 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
			>
				<ArrowRightIcon className="size-4" />
			</button>
			<input
				type="text"
				aria-label="Address"
				value={shown}
				onChange={(e) => setDraft(e.target.value)}
				onKeyDown={(e) => {
					if (e.key === "Enter") submit();
				}}
				onMouseDown={(e) => {
					if (document.activeElement === e.currentTarget) return;
					// WebKit places the caret in mousedown's own default action, which
					// collapses whatever the focus handler selected — suppressing the
					// later mouseup is too late. Take focus by hand instead; onFocus
					// then selects. A second click lands here already focused and gets
					// normal caret placement.
					e.preventDefault();
					e.currentTarget.focus();
				}}
				onFocus={(e) => {
					// Every browser selects the whole address on focus: the next
					// keystroke replaces the URL rather than inserting into it.
					e.currentTarget.select();
				}}
				onBlur={() => setDraft(null)}
				spellCheck={false}
				autoCorrect="off"
				autoCapitalize="off"
				className="h-7 min-w-0 flex-1 cursor-text rounded-md border bg-surface px-2 font-mono text-xs text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
			/>
			{children}
		</div>
	);
}
