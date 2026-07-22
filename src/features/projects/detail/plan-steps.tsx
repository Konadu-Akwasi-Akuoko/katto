import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import type { FailureKind, StageName } from "@/lib/ipc/pipeline";
import { cn } from "@/lib/utils";
import type { PipelineRun, StepState } from "@/stores/pipeline";

const STEPS: { stage: StageName; label: string }[] = [
	{ stage: "extracting_audio", label: "Extracting audio" },
	{ stage: "transcribing", label: "Transcribing" },
	{ stage: "detecting_cuts", label: "Detecting cuts" },
];

/** m:ss elapsed since `startedAt`. */
function elapsedLabel(startedAt: number, now: number): string {
	const total = Math.max(0, Math.floor((now - startedAt) / 1000));
	const m = Math.floor(total / 60);
	const s = total % 60;
	return `${m}:${String(s).padStart(2, "0")}`;
}

/** Plain-language failure copy: the kind names the fix, not just the fault. */
function failureCopy(error: string, kind: FailureKind | null): string {
	if (kind === "auth") {
		return "The API key was rejected — re-enter it in Settings.";
	}
	if (kind === "quota") {
		return "Rate limited — try again in a few minutes.";
	}
	if (kind === "invalid_output") {
		return "The planner never produced a valid cut plan — retry, or switch planner in Settings.";
	}
	if (error.includes("claude subprocess")) {
		return "claude exited with an error — open Settings to add an Anthropic API key instead.";
	}
	return error;
}

function dotClass(state: StepState): string {
	switch (state) {
		case "pending":
			return "bg-queued";
		case "active":
			return "bg-ember motion-safe:animate-pulse";
		case "done":
			return "bg-done";
		case "failed":
			return "bg-failed";
	}
}

/**
 * The three-step rough-cut indicator inside the footage card: state dot +
 * label + right-aligned machine detail (elapsed timer, live cut count).
 * State is shown once, on the dot — no rails, no rings.
 */
export function PlanSteps({
	run,
	onReview,
}: {
	run: PipelineRun;
	onReview?: (bundlePath: string) => void;
}) {
	const [now, setNow] = useState(() => Date.now());
	const running = !run.finished;

	useEffect(() => {
		if (!running) return;
		const timer = setInterval(() => setNow(Date.now()), 1000);
		return () => clearInterval(timer);
	}, [running]);

	function detail(stage: StageName, state: StepState): string {
		if (state === "pending") return "—";
		if (state !== "active") return "";
		if (stage === "detecting_cuts" && run.cutsSoFar.length > 0) {
			const n = run.cutsSoFar.length;
			return `${n} cut${n === 1 ? "" : "s"}`;
		}
		return elapsedLabel(run.startedAt, now);
	}

	return (
		<div className="flex flex-col">
			<ul className="flex flex-col">
				{STEPS.map(({ stage, label }, i) => {
					const state = run.steps[stage];
					return (
						<li key={stage} className="relative flex flex-col">
							<div className="flex h-[30px] items-center gap-2">
								<span className="relative flex w-2 flex-col items-center self-stretch">
									{i > 0 && (
										<span
											aria-hidden
											className="absolute top-0 h-[calc(50%-4px)] w-px bg-hairline"
										/>
									)}
									<span
										data-state={state}
										className={cn(
											"absolute top-1/2 size-2 -translate-y-1/2 rounded-full",
											dotClass(state),
										)}
									/>
									{i < STEPS.length - 1 && (
										<span
											aria-hidden
											className="absolute bottom-0 h-[calc(50%-4px)] w-px bg-hairline"
										/>
									)}
								</span>
								<span className="flex-1 text-sm">{label}</span>
								<span className="font-mono text-xs tabular-nums text-fg-muted">
									{detail(stage, state)}
								</span>
							</div>
							{state === "active" && run.stageProgress > 0 && (
								<div className="mb-1 ml-4 h-1 overflow-hidden rounded-full bg-hairline">
									<div
										className="h-full bg-ember transition-[width] duration-300"
										style={{ width: `${Math.round(run.stageProgress * 100)}%` }}
									/>
								</div>
							)}
						</li>
					);
				})}
			</ul>
			{run.error !== null && (
				<p className="mt-1 text-sm text-failed">
					{failureCopy(run.error, run.errorKind)}
				</p>
			)}
			{run.error === null && run.bundlePath !== null && (
				<div className="mt-1">
					<Button
						variant="secondary"
						size="sm"
						className="cursor-default"
						onClick={() => run.bundlePath && onReview?.(run.bundlePath)}
					>
						{run.finished ? "Review cut plan" : "Review transcript"}
					</Button>
				</div>
			)}
		</div>
	);
}
