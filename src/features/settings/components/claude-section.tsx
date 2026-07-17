import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { detectClaude } from "@/lib/ipc/onboarding";
import type { Settings } from "@/lib/ipc/settings";
import { settingsKeys } from "@/lib/ipc/settings";

export function ClaudeSection({ settings }: { settings: Settings }) {
	const queryClient = useQueryClient();
	const detect = useMutation({
		mutationFn: detectClaude,
		onSuccess: () =>
			queryClient.invalidateQueries({ queryKey: settingsKeys.all }),
	});

	return (
		<section className="flex flex-col gap-3">
			<h2 className="font-serif text-lg font-semibold">Claude</h2>
			<div className="flex items-center justify-between gap-4 text-sm">
				{settings.claude_path ? (
					<span className="font-mono text-fg-muted">
						{settings.claude_path}
					</span>
				) : (
					<span className="text-fg-muted">Not found on PATH</span>
				)}
				<Button
					variant="secondary"
					onClick={() => detect.mutate()}
					disabled={detect.isPending}
				>
					Re-run detection
				</Button>
			</div>
		</section>
	);
}
