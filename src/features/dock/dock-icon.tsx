import { CheckIcon, RobotIcon } from "@phosphor-icons/react";
import { useSessions } from "@/features/dock/use-sessions";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";
import { deriveDockIconState } from "./model/dock-state";
import { useDoneFlash } from "./use-done-flash";

/**
 * The sidebar Claude entry (the PRD's "dock icon"): running → pulsing ember
 * ring, needs-input → warn badge dot (one encoding at a time), and a ~3 s
 * check flash when a previously-running session settles.
 */
export function DockIcon() {
	const dockOpen = useUiStore((s) => s.dockOpen);
	const toggleDock = useUiStore((s) => s.toggleDock);
	const { data: sessions = [] } = useSessions();
	const iconState = deriveDockIconState(sessions);

	const doneFlash = useDoneFlash(sessions);

	return (
		<button
			type="button"
			aria-pressed={dockOpen}
			onClick={toggleDock}
			className={cn(
				"flex cursor-default items-center gap-2.5 rounded-md px-2 py-1.5 text-sm transition-colors",
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
			Claude dock
		</button>
	);
}
