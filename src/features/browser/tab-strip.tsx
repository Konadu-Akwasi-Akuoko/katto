import { PlusIcon, XIcon } from "@phosphor-icons/react";
import type { TabSnapshot } from "@/lib/ipc/browser";
import { cn } from "@/lib/utils";

/**
 * Compact 32px tab row: active tab = surface step + hairline, never
 * accent-colored. Titles are URL-derived (or the page title when WKWebView
 * reported one); no favicons — the globe stays honest.
 */
export function TabStrip({
	tabs,
	activeId,
	onSelect,
	onClose,
	onNew,
}: {
	tabs: TabSnapshot[];
	activeId: number | null;
	onSelect: (id: number) => void;
	onClose: (id: number) => void;
	onNew: () => void;
}) {
	return (
		<div
			role="tablist"
			aria-label="Browser tabs"
			className="flex h-8 shrink-0 items-center gap-1 overflow-x-auto border-b px-2"
		>
			{tabs.map((tab) => {
				const active = tab.id === activeId;
				return (
					<div
						key={tab.id}
						className={cn(
							"group flex h-7 max-w-52 min-w-24 items-center gap-1 rounded-md border px-2",
							active
								? "border-border bg-surface-2 text-fg"
								: "border-transparent text-fg-muted hover:bg-surface-2 hover:text-fg",
						)}
					>
						<button
							type="button"
							role="tab"
							aria-selected={active}
							onClick={() => onSelect(tab.id)}
							className="min-w-0 flex-1 truncate text-left text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
							title={tab.url}
						>
							{tab.title}
						</button>
						<button
							type="button"
							aria-label={`Close ${tab.title}`}
							onClick={() => onClose(tab.id)}
							className={cn(
								"rounded-sm p-0.5 text-fg-faint hover:bg-surface hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2",
								active ? "" : "opacity-0 group-hover:opacity-100",
							)}
						>
							<XIcon className="size-3" />
						</button>
					</div>
				);
			})}
			<button
				type="button"
				aria-label="New tab"
				onClick={onNew}
				className="flex size-7 shrink-0 items-center justify-center rounded-md text-fg-muted hover:bg-surface-2 hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
			>
				<PlusIcon className="size-4" />
			</button>
		</div>
	);
}
