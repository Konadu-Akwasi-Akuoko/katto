import { useDriveStatus } from "@/hooks/use-drive-status";

/**
 * App-wide strip shown while the studio root is unreachable. Lives in the
 * AppShell's fixed frame (above the scroll pane); the watcher clears it the
 * moment the drive comes back.
 */
export function DriveBanner() {
	const { data } = useDriveStatus();
	if (!data || data.mounted || data.path === null) return null;
	return (
		<div
			role="alert"
			className="flex items-center gap-2 border-warn/40 border-b bg-warn/10 px-4 py-1.5 text-sm"
			style={{ backgroundImage: "none" }}
		>
			<span className="size-1.5 shrink-0 rounded-full bg-warn" />
			<span>
				Studio drive disconnected — plug the drive back in and katto will pick it up.
			</span>
			<span className="ml-auto font-mono text-fg-muted text-xs">{data.path}</span>
		</div>
	);
}
