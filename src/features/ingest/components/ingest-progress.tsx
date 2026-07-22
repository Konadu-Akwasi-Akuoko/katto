import { CheckCircleIcon, EjectIcon } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { clipCountLabel } from "@/features/ingest/model/select";
import { ejectCard } from "@/lib/ipc/ingest";
import type { JobProgress } from "@/lib/ipc/jobs";
import { subscribeJobProgress } from "@/lib/ipc/jobs";

interface Props {
	jobId: string;
	volume: string;
	projectTitle: string;
	clipCount: number;
}

/**
 * The macOS copy-sheet-styled ingest panel: one 4px bar, a count line, and an
 * Eject affordance once the copy completes. Failures keep the last streamed
 * message visible; the shared jobs list carries the failed state.
 */
export function IngestProgress({
	jobId,
	volume,
	projectTitle,
	clipCount,
}: Props) {
	const [progress, setProgress] = useState(0);
	const [message, setMessage] = useState<string | null>(null);
	const [done, setDone] = useState(false);

	useEffect(() => {
		let cancelled = false;
		void subscribeJobProgress(jobId, (update: JobProgress) => {
			if (cancelled) return;
			setProgress(update.progress);
			setMessage(update.message);
			if (update.progress >= 1) setDone(true);
		});
		return () => {
			cancelled = true;
		};
	}, [jobId]);

	const eject = useMutation({
		mutationFn: () => ejectCard(volume),
		onSuccess: () => toast.success("Card ejected — safe to remove"),
		onError: (err) => toast.error(err.message),
	});

	return (
		<div className="flex flex-col gap-2 rounded-[var(--r-lg)] border border-border bg-surface p-4">
			<div className="flex items-center gap-2">
				{done ? (
					<CheckCircleIcon size={16} className="text-done" weight="fill" />
				) : null}
				<span className="text-sm">
					{done
						? `Imported ${clipCountLabel(clipCount)} → ${projectTitle}`
						: `Copying ${clipCountLabel(clipCount)} → ${projectTitle}`}
				</span>
			</div>
			<Progress value={progress * 100} />
			{message && !done ? (
				<span className="font-mono text-fg-muted text-xs tabular-nums">
					{message}
				</span>
			) : null}
			{done ? (
				<div className="flex justify-end">
					<Button
						variant="secondary"
						onClick={() => eject.mutate()}
						disabled={eject.isPending}
					>
						<EjectIcon size={16} /> Eject card
					</Button>
				</div>
			) : null}
		</div>
	);
}
