import { CheckCircleIcon, EjectIcon, WarningIcon } from "@phosphor-icons/react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { clipCountLabel, remainingClips } from "@/features/ingest/model/select";
import { eventsKeys, listEvents } from "@/lib/ipc/events";
import { ejectCard } from "@/lib/ipc/ingest";
import type { JobProgress } from "@/lib/ipc/jobs";
import { jobsKeys, listJobs, subscribeJobProgress } from "@/lib/ipc/jobs";

interface Props {
	jobId: string;
	volume: string;
	projectTitle: string;
	clipCount: number;
	/** When set and the job failed with a known remainder, the panel offers
	 * "Retry remaining N clips" and calls back with those source paths. */
	onRetry?: (remaining: string[]) => void;
}

/**
 * The macOS copy-sheet-styled ingest panel: one 4px bar, a count line, Eject
 * once the copy completes, and — for a failed job — the terminal error plus a
 * retry-remainder affordance fed by the job's `ingest_failed` events row.
 */
export function IngestProgress({
	jobId,
	volume,
	projectTitle,
	clipCount,
	onRetry,
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

	// The job row is the durable status source (the channel is best-effort).
	const { data: jobs = [] } = useQuery({
		queryKey: jobsKeys.all,
		queryFn: () => listJobs(false),
	});
	const job = jobs.find((j) => j.id === jobId);
	const failed = job?.status === "failed";
	const errorMessage = job?.error ?? (failed ? message : null);

	const { data: events = [] } = useQuery({
		queryKey: eventsKeys.all,
		queryFn: () => listEvents(50),
		enabled: failed,
	});
	const remaining = failed ? remainingClips(events, jobId) : null;

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
				{failed ? (
					<WarningIcon size={16} className="text-failed" weight="fill" />
				) : null}
				<span className="text-sm">
					{done
						? `Imported ${clipCountLabel(clipCount)} → ${projectTitle}`
						: failed
							? `Import into ${projectTitle} failed`
							: `Copying ${clipCountLabel(clipCount)} → ${projectTitle}`}
				</span>
			</div>
			<Progress value={progress * 100} />
			{failed && errorMessage ? (
				<span className="text-failed text-xs">{errorMessage}</span>
			) : message && !done ? (
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
			{failed && onRetry && remaining && remaining.length > 0 ? (
				<div className="flex justify-end">
					<Button variant="secondary" onClick={() => onRetry(remaining)}>
						Retry remaining {clipCountLabel(remaining.length)}
					</Button>
				</div>
			) : null}
		</div>
	);
}
