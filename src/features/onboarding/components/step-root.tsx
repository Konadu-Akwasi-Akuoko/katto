import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { canContinue, rootWarnings } from "@/lib/root-warnings";
import { pickStudioRoot, type RootCheck } from "@/lib/ipc/onboarding";
import { patchSettings } from "@/lib/ipc/settings";

export function StepRoot({ onDone }: { onDone: () => void }) {
	const [check, setCheck] = useState<RootCheck | null>(null);
	const warnings = check ? rootWarnings(check) : [];

	const pick = useMutation({
		mutationFn: pickStudioRoot,
		onSuccess: (picked) => {
			if (picked) setCheck(picked);
		},
	});
	const save = useMutation({
		mutationFn: (path: string) => patchSettings({ studio_root: path }),
		onSuccess: onDone,
	});

	return (
		<section className="flex flex-col gap-6">
			<div className="flex flex-col gap-2">
				<h1 className="font-serif text-3xl font-semibold">Where does your footage live?</h1>
				<p className="text-fg-muted">
					katto keeps every project as plain folders under one studio root. An external
					SSD is the comfortable choice; any folder works.
				</p>
			</div>
			<div className="flex items-center gap-3">
				<Button variant="secondary" onClick={() => pick.mutate()} disabled={pick.isPending}>
					Choose folder…
				</Button>
				{check && <span className="font-mono text-sm text-fg-muted">{check.path}</span>}
			</div>
			{warnings.length > 0 && (
				<ul className="flex flex-col gap-1 text-sm text-warn">
					{warnings.map((warning) => (
						<li key={warning}>{warning}</li>
					))}
				</ul>
			)}
			<div>
				<Button
					onClick={() => check && save.mutate(check.path)}
					disabled={!canContinue(check) || save.isPending}
				>
					Continue
				</Button>
			</div>
		</section>
	);
}
