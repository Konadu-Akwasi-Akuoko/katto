import { XIcon } from "@phosphor-icons/react";
import type { KeyboardEvent } from "react";
import type { SessionInfo } from "@/lib/ipc/sessions";
import { cn } from "@/lib/utils";
import { tabNote } from "./model/dock-state";

const dotClass: Record<SessionInfo["state"]["kind"], string> = {
	running: "bg-ember",
	needs_input: "bg-warn",
	idle: "bg-done",
	failed: "bg-failed",
	closed: "bg-fg-faint",
};

export function TabStrip({
	sessions,
	activeId,
	onSelect,
	onClose,
}: {
	sessions: SessionInfo[];
	activeId: string | null;
	onSelect: (id: string) => void;
	onClose: (id: string) => void;
}) {
	if (sessions.length === 0) return null;
	return (
		<div
			role="tablist"
			aria-label="Claude sessions"
			className="flex shrink-0 gap-1 overflow-x-auto px-3 pb-2"
		>
			{sessions.map((session) => {
				const active = session.id === activeId;
				const note = tabNote(session);
				const handleKey = (event: KeyboardEvent) => {
					if (event.key === "Enter" || event.key === " ") {
						event.preventDefault();
						onSelect(session.id);
					}
				};
				return (
					// The close control is a real <button>; nesting it in a <button>
					// tab would be invalid HTML, so the tab is a div with tab semantics.
					<div
						key={session.id}
						role="tab"
						aria-selected={active}
						tabIndex={0}
						onClick={() => onSelect(session.id)}
						onKeyDown={handleKey}
						className={cn(
							"flex h-8 min-w-0 cursor-default items-center gap-2 rounded-md border px-2.5 text-sm select-none",
							active
								? "border-border bg-surface-2 text-fg"
								: "border-transparent text-fg-muted hover:bg-surface-2 hover:text-fg",
						)}
					>
						<span
							aria-hidden
							className={cn(
								"size-1.5 shrink-0 rounded-full",
								dotClass[session.state.kind],
							)}
						/>
						<span className="truncate">{session.label}</span>
						{note !== null && (
							<span
								className="max-w-40 truncate text-xs text-fg-faint"
								title={note}
							>
								{note}
							</span>
						)}
						<button
							type="button"
							aria-label={`close ${session.label}`}
							onClick={(event) => {
								event.stopPropagation();
								onClose(session.id);
							}}
							className="cursor-default rounded-sm p-0.5 text-fg-faint hover:bg-surface hover:text-fg"
						>
							<XIcon className="size-3.5" />
						</button>
					</div>
				);
			})}
		</div>
	);
}
