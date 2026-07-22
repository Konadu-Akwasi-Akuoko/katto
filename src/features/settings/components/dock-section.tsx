import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { parseSpecTime } from "@/features/settings/model/schedule-spec";
import {
	getSchedulerState,
	runScheduledJobNow,
	schedulerKeys,
	setScheduledJob,
} from "@/lib/ipc/scheduler";
import type { Settings } from "@/lib/ipc/settings";
import { patchSettings, settingsKeys } from "@/lib/ipc/settings";

const CURATION_JOB = "nightly-curation";
const REAP_CHOICES = [2, 5, 10] as const;

export function DockSection({ settings }: { settings: Settings }) {
	const queryClient = useQueryClient();
	const patch = useMutation({
		mutationFn: patchSettings,
		onSuccess: () =>
			queryClient.invalidateQueries({ queryKey: settingsKeys.all }),
	});
	const scheduler = useQuery({
		queryKey: schedulerKeys.all,
		queryFn: getSchedulerState,
	});
	const curation = scheduler.data?.find((job) => job.name === CURATION_JOB);
	const time = curation ? parseSpecTime(curation.spec) : null;

	const updateSchedule = useMutation({
		mutationFn: ({
			hour,
			minute,
			enabled,
		}: {
			hour: number;
			minute: number;
			enabled: boolean;
		}) => setScheduledJob(CURATION_JOB, hour, minute, enabled),
		onSuccess: () =>
			queryClient.invalidateQueries({ queryKey: schedulerKeys.all }),
	});
	const runNow = useMutation({
		mutationFn: () => runScheduledJobNow(CURATION_JOB),
	});

	return (
		<section className="flex flex-col gap-4">
			<h2 className="font-serif text-lg font-semibold">Claude dock</h2>

			<div className="flex items-center justify-between gap-4 text-sm">
				<Label htmlFor="idle-reap">Idle sessions close after</Label>
				<Select
					value={String(settings.idle_reap_minutes)}
					onValueChange={(value) =>
						patch.mutate({ idle_reap_minutes: Number(value) })
					}
				>
					<SelectTrigger id="idle-reap" className="w-32">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						{REAP_CHOICES.map((minutes) => (
							<SelectItem key={minutes} value={String(minutes)}>
								{minutes} minutes
							</SelectItem>
						))}
					</SelectContent>
				</Select>
			</div>

			<div className="flex items-center justify-between gap-4 text-sm">
				<Label htmlFor="dock-planning">Cut planning in the dock</Label>
				<Switch
					id="dock-planning"
					checked={settings.dock_planning}
					onCheckedChange={(v) => patch.mutate({ dock_planning: v })}
					disabled={patch.isPending}
				/>
			</div>

			<div className="flex flex-col gap-2">
				<div className="flex items-center justify-between gap-4 text-sm">
					<Label htmlFor="curation-enabled">Nightly curation</Label>
					<Switch
						id="curation-enabled"
						checked={curation?.enabled ?? false}
						onCheckedChange={(enabled) =>
							updateSchedule.mutate({
								hour: time?.hour ?? 0,
								minute: time?.minute ?? 0,
								enabled,
							})
						}
						disabled={curation === undefined || updateSchedule.isPending}
					/>
				</div>
				<div className="flex items-center justify-between gap-4 text-sm">
					<Label htmlFor="curation-time">Curation time</Label>
					<div className="flex items-center gap-2">
						<Input
							id="curation-time"
							type="time"
							className="w-28 tabular-nums"
							key={curation?.spec}
							defaultValue={
								time
									? `${String(time.hour).padStart(2, "0")}:${String(time.minute).padStart(2, "0")}`
									: ""
							}
							onBlur={(e) => {
								const [hh, mm] = e.target.value.split(":");
								const hour = Number(hh);
								const minute = Number(mm);
								if (
									Number.isInteger(hour) &&
									Number.isInteger(minute) &&
									(hour !== time?.hour || minute !== time?.minute)
								) {
									updateSchedule.mutate({
										hour,
										minute,
										enabled: curation?.enabled ?? true,
									});
								}
							}}
						/>
						<Button
							variant="secondary"
							size="sm"
							onClick={() => runNow.mutate()}
							disabled={runNow.isPending}
						>
							Run now
						</Button>
					</div>
				</div>
				<p className="text-xs text-fg-muted">
					{curation?.last_success_at
						? `last ran ${curation.last_success_at}`
						: "never ran"}
				</p>
			</div>

			<div className="flex flex-col gap-2">
				<div className="flex items-center justify-between gap-4 text-sm">
					<Label htmlFor="discovery-enabled">Discovery</Label>
					<Switch
						id="discovery-enabled"
						checked={settings.discovery_enabled}
						onCheckedChange={(v) => patch.mutate({ discovery_enabled: v })}
						disabled={patch.isPending}
					/>
				</div>
				<div className="flex items-center justify-between gap-4 text-sm">
					<Label htmlFor="hyperframes-path">hyper-frames checkout</Label>
					<Input
						id="hyperframes-path"
						className="w-64"
						placeholder="/path/to/hyper-frames"
						defaultValue={settings.hyperframes_path ?? ""}
						onBlur={(e) => {
							const value = e.target.value.trim();
							if (value !== (settings.hyperframes_path ?? "")) {
								patch.mutate({ hyperframes_path: value });
							}
						}}
					/>
				</div>
				<p className="text-xs text-fg-muted">
					Pulls fresh signals before curation. Needs uv and a hyper-frames
					checkout.
				</p>
			</div>
		</section>
	);
}
