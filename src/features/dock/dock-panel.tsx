import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { Button } from "@/components/ui/button";
import { useSessions } from "@/features/dock/use-sessions";
import {
	closeSession,
	sessionsKeys,
	setDockFocus,
	spawnSession,
} from "@/lib/ipc/sessions";
import { getSettings, settingsKeys } from "@/lib/ipc/settings";
import { useUiStore } from "@/stores/ui";
import { TabStrip } from "./tab-strip";
import { Terminal } from "./terminal";

/**
 * The Claude dock: a right-side slide-over inside the content pane. Mounted
 * once (app shell overlay slot); renders nothing while hidden but keeps
 * reporting dock focus so the backend's reap exemption and notification
 * suppression stay correct.
 */
export function DockPanel() {
	const dockOpen = useUiStore((s) => s.dockOpen);
	const activeSessionId = useUiStore((s) => s.activeSessionId);
	const setActiveSession = useUiStore((s) => s.setActiveSession);
	const closeDock = useUiStore((s) => s.closeDock);
	const queryClient = useQueryClient();

	const { data: sessions = [] } = useSessions();
	const { data: settings } = useQuery({
		queryKey: settingsKeys.all,
		queryFn: getSettings,
		enabled: dockOpen,
	});

	const active =
		sessions.find((s) => s.id === activeSessionId) ??
		sessions[sessions.length - 1] ??
		null;
	const activeId = active?.id ?? null;

	useEffect(() => {
		// Focus reporting is a hint (reap exemption, notification routing) —
		// a lost report self-heals on the next transition, so failures only log.
		setDockFocus(dockOpen, dockOpen ? activeId : null).catch(() => {});
	}, [dockOpen, activeId]);

	const spawn = useMutation({
		mutationFn: () =>
			spawnSession({
				label: "session",
				cwd: settings?.studio_root ?? "/",
				initial_prompt: null,
			}),
		onSuccess: (id) => {
			setActiveSession(id);
			void queryClient.invalidateQueries({ queryKey: sessionsKeys.all });
		},
	});

	const close = useMutation({
		mutationFn: (id: string) => closeSession(id),
		onSuccess: () => {
			void queryClient.invalidateQueries({ queryKey: sessionsKeys.all });
		},
	});

	if (!dockOpen) return null;

	return (
		<section
			aria-label="Claude dock"
			className="motion-reduce:animate-none animate-in slide-in-from-right absolute inset-y-0 right-0 z-20 flex w-[600px] max-w-[80vw] flex-col border-l border-hairline bg-surface shadow-(--shadow) duration-(--dur-slow) ease-(--ease) grain"
		>
			<header className="flex h-12 shrink-0 items-center gap-2 px-3">
				<span className="font-serif text-base text-fg">Claude</span>
				<div className="ml-auto flex items-center gap-1.5">
					<Button
						variant="secondary"
						size="sm"
						disabled={spawn.isPending}
						onClick={() => spawn.mutate()}
					>
						New session
					</Button>
					<Button variant="ghost" size="sm" onClick={closeDock}>
						Hide
					</Button>
				</div>
			</header>
			<TabStrip
				sessions={sessions}
				activeId={activeId}
				onSelect={setActiveSession}
				onClose={(id) => close.mutate(id)}
			/>
			<div className="min-h-0 flex-1 px-3 pb-3">
				{active === null ? (
					<p className="pt-6 text-center text-sm text-fg-muted">
						No sessions. AI tasks open a visible session here — you can type
						into any of them.
					</p>
				) : (
					<Terminal key={active.id} sessionId={active.id} />
				)}
			</div>
		</section>
	);
}
