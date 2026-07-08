import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import {
	devRunSmokeJob,
	jobsKeys,
	listJobs,
	subscribeJobProgress,
} from "@/lib/ipc/jobs";
import type { Job, JobProgress } from "@/lib/ipc/jobs";

const CHIP_VARIANTS = ["running", "done", "failed", "queued"] as const;
type ChipVariant = (typeof CHIP_VARIANTS)[number];

function chipVariant(status: string): ChipVariant {
	return CHIP_VARIANTS.find((variant) => variant === status) ?? "queued";
}

function JobRow({ job }: { job: Job }) {
	const [live, setLive] = useState<JobProgress | null>(null);
	useEffect(() => {
		let cancelled = false;
		void subscribeJobProgress(job.id, (update) => {
			if (!cancelled) setLive(update);
		});
		return () => {
			cancelled = true;
		};
	}, [job.id]);

	const progress = live?.progress ?? job.progress;
	return (
		<div className="flex flex-col gap-1.5">
			<div className="flex items-center justify-between gap-2">
				<span className="truncate text-sm">{job.label}</span>
				<Badge variant={chipVariant(job.status)}>
					<span className="dot" />
					{job.status}
				</Badge>
			</div>
			<Progress value={progress * 100} className="h-1" />
			{live?.message !== undefined && live.message !== null && (
				<span className="text-fg-faint text-xs">{live.message}</span>
			)}
		</div>
	);
}

export function ActiveJobs() {
	const queryClient = useQueryClient();
	const { data: jobs } = useQuery({
		queryKey: jobsKeys.active,
		queryFn: () => listJobs(true),
	});
	const smoke = useMutation({
		mutationFn: () => devRunSmokeJob(false),
		onSuccess: () => queryClient.invalidateQueries({ queryKey: jobsKeys.all }),
	});
	return (
		<Card>
			<CardHeader>
				<CardTitle>Jobs</CardTitle>
			</CardHeader>
			<CardContent className="flex flex-col gap-4">
				{jobs === undefined || jobs.length === 0 ? (
					<p className="text-fg-muted text-sm">Nothing running.</p>
				) : (
					jobs.map((job) => <JobRow key={job.id} job={job} />)
				)}
				{import.meta.env.DEV && (
					<Button
						variant="outline"
						size="sm"
						className="self-start"
						disabled={smoke.isPending}
						onClick={() => smoke.mutate()}
					>
						Run smoke job
					</Button>
				)}
			</CardContent>
		</Card>
	);
}
