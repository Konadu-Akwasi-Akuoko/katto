import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import type { RootCheck } from "@/lib/ipc/onboarding";
import { pickStudioRoot } from "@/lib/ipc/onboarding";
import type { Settings } from "@/lib/ipc/settings";
import { patchSettings, settingsKeys } from "@/lib/ipc/settings";
import { rootWarnings } from "@/lib/root-warnings";

export function StudioRootSection({ settings }: { settings: Settings }) {
	const queryClient = useQueryClient();
	const [lastCheck, setLastCheck] = useState<RootCheck | null>(null);

	const save = useMutation({
		mutationFn: (path: string) => patchSettings({ studio_root: path }),
		onSuccess: () => queryClient.invalidateQueries({ queryKey: settingsKeys.all }),
	});
	const repick = useMutation({
		mutationFn: pickStudioRoot,
		onSuccess: (check) => {
			if (!check) return;
			setLastCheck(check);
			if (check.writable) save.mutate(check.path);
		},
	});

	const warnings = lastCheck ? rootWarnings(lastCheck) : [];

	return (
		<section className="flex flex-col gap-3">
			<h2 className="font-serif text-lg font-semibold">Studio root</h2>
			<div className="flex items-center justify-between gap-4 text-sm">
				{settings.studio_root ? (
					<span className="font-mono text-fg-muted">{settings.studio_root}</span>
				) : (
					<span className="text-fg-muted">Not set</span>
				)}
				<Button
					variant="secondary"
					onClick={() => repick.mutate()}
					disabled={repick.isPending || save.isPending}
				>
					Change…
				</Button>
			</div>
			{warnings.length > 0 && (
				<ul className="flex flex-col gap-1 text-sm text-warn">
					{warnings.map((warning) => (
						<li key={warning}>{warning}</li>
					))}
				</ul>
			)}
		</section>
	);
}
