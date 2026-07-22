import { ArrowLeftIcon, ArrowRightIcon } from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import type { TabSnapshot } from "@/lib/ipc/browser";
import { cn } from "@/lib/utils";
import { displayUrl, normalizeAddress } from "./model/address";

/**
 * Back/forward + address bar. The input is mono (a URL is machine data);
 * a non-address entry shows the inline hint instead of searching — katto is
 * not a search engine.
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
	const [draft, setDraft] = useState("");
	const [hint, setHint] = useState(false);
	const currentUrl = activeTab?.url ?? "";

	// follow navigation while the user isn't mid-edit
	useEffect(() => {
		setDraft(currentUrl === "" ? "" : displayUrl(currentUrl));
		setHint(false);
	}, [currentUrl]);

	function submit() {
		const normalized = normalizeAddress(draft);
		if (normalized === null) {
			setHint(true);
			return;
		}
		setHint(false);
		onNavigate(normalized);
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
			<div className="relative min-w-0 flex-1">
				<input
					type="text"
					aria-label="Address"
					value={draft}
					onChange={(e) => {
						setDraft(e.target.value);
						setHint(false);
					}}
					onKeyDown={(e) => {
						if (e.key === "Enter") submit();
					}}
					spellCheck={false}
					autoCorrect="off"
					autoCapitalize="off"
					className={cn(
						"h-7 w-full cursor-text rounded-md border bg-surface px-2 font-mono text-xs text-fg",
						"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2",
						hint && "border-warn",
					)}
				/>
				{hint && (
					<span className="absolute top-full left-1 mt-0.5 text-[11px] text-warn">
						Enter a full address
					</span>
				)}
			</div>
			{children}
		</div>
	);
}
