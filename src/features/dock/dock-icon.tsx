import { CheckIcon, RobotIcon } from "@phosphor-icons/react";
import { useSessions } from "@/features/dock/use-sessions";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";
import { deriveDockIconState } from "./model/dock-state";
import { useDoneFlash } from "./use-done-flash";

/**
 * The Claude entry (the PRD's "dock icon"): running → pulsing ember ring,
 * needs-input → warn badge dot (one encoding at a time), and a ~3 s check flash
 * when a previously-running session settles.
 *
 * `compact` drops the text label for the titlebar, where this has to appear
 * whenever the sidebar is collapsed — a background session asking for input is
 * the one thing that must never go unannounced because a panel is hidden.
 */
export function DockIcon({ compact = false }: { compact?: boolean }) {
	const dockOpen = useUiStore((s) => s.dockOpen);
	const toggleDock = useUiStore((s) => s.toggleDock);
	const { data: sessions = [] } = useSessions();
	const iconState = deriveDockIconState(sessions);

	const doneFlash = useDoneFlash(sessions);

	return (
		<button
			type="button"
			aria-pressed={dockOpen}
			aria-label={compact ? "Claude dock" : undefined}
			onClick={toggleDock}
			className={cn(
				"flex cursor-default items-center rounded-md transition-colors",
				compact ? "size-8 justify-center" : "gap-2.5 px-2 py-1.5 text-sm",
				dockOpen
					? "bg-surface-2 text-fg"
					: "text-fg-muted hover:bg-surface-2 hover:text-fg",
			)}
		>
			<span className="relative inline-flex size-4 items-center justify-center">
				<RobotIcon className="size-4" />
				{iconState === "running" && !doneFlash && (
					<span
						aria-hidden
						className="motion-reduce:animate-none absolute -inset-1 animate-pulse rounded-full ring-2 ring-ember"
					/>
				)}
				{iconState === "needs-input" && (
					<span
						aria-hidden
						className="absolute -top-0.5 -right-0.5 size-1.5 rounded-full bg-warn"
					/>
				)}
				{doneFlash && iconState !== "needs-input" && (
					<CheckIcon
						aria-hidden
						weight="bold"
						className="absolute -top-1 -right-1 size-2.5 text-done"
					/>
				)}
			</span>
			{!compact && "Claude dock"}
		</button>
	);
}
