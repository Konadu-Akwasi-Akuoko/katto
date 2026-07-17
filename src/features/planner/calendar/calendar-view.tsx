import {
	CameraIcon,
	CaretLeftIcon,
	CaretRightIcon,
	UploadSimpleIcon,
} from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type {
	CalendarCell,
	CalendarMode,
} from "@/features/planner/model/calendar";
import {
	addDaysIso,
	addMonthsIso,
	chipsByDate,
	monthGrid,
	periodLabel,
	weekRow,
} from "@/features/planner/model/calendar";
import { listProjects, projectsKeys } from "@/lib/ipc/projects";
import type { ScheduleEntry } from "@/lib/ipc/schedule";
import { listSchedule, scheduleKeys } from "@/lib/ipc/schedule";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/** Today as a local calendar date (`YYYY-MM-DD`), matching the schedule's dates. */
function todayIso(): string {
	const now = new Date();
	const pad = (n: number) => String(n).padStart(2, "0");
	return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

export function CalendarView() {
	const openProject = useUiStore((s) => s.openProject);
	const [mode, setMode] = useState<CalendarMode>("month");
	const [anchor, setAnchor] = useState(todayIso);
	const today = todayIso();

	const weeks = useMemo<CalendarCell[][]>(() => {
		if (mode === "week") return [weekRow(anchor)];
		const [year, month] = anchor.split("-").map(Number);
		return monthGrid(year ?? 0, (month ?? 1) - 1);
	}, [mode, anchor]);

	const cells = useMemo(() => weeks.flat(), [weeks]);
	const from = cells[0]?.iso ?? anchor;
	const to = cells[cells.length - 1]?.iso ?? anchor;

	const { data: entries } = useQuery({
		queryKey: scheduleKeys.range(from, to),
		queryFn: () => listSchedule(from, to),
	});
	const { data: projects } = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});

	const chips = useMemo(() => chipsByDate(entries ?? []), [entries]);
	const titleOf = useMemo(() => {
		const map = new Map(projects?.map((p) => [p.slug, p.title]));
		return (slug: string) => map.get(slug) ?? slug;
	}, [projects]);

	function step(delta: number) {
		setAnchor((current) =>
			mode === "week"
				? addDaysIso(current, delta * 7)
				: addMonthsIso(current, delta),
		);
	}

	return (
		<div className="flex h-full min-h-0 flex-col gap-3">
			<header className="flex items-center gap-3">
				<h2 className="font-serif text-lg text-fg">
					{periodLabel(anchor, mode)}
				</h2>
				<div className="flex items-center gap-1">
					<Button
						variant="ghost"
						size="icon-sm"
						aria-label="Previous"
						onClick={() => step(-1)}
					>
						<CaretLeftIcon />
					</Button>
					<Button
						variant="ghost"
						size="sm"
						onClick={() => setAnchor(todayIso())}
					>
						Today
					</Button>
					<Button
						variant="ghost"
						size="icon-sm"
						aria-label="Next"
						onClick={() => step(1)}
					>
						<CaretRightIcon />
					</Button>
				</div>
				<div className="ml-auto flex items-center gap-1">
					<Button
						variant={mode === "month" ? "secondary" : "ghost"}
						size="sm"
						onClick={() => setMode("month")}
					>
						Month
					</Button>
					<Button
						variant={mode === "week" ? "secondary" : "ghost"}
						size="sm"
						onClick={() => setMode("week")}
					>
						Week
					</Button>
				</div>
			</header>

			<div className="grid grid-cols-7 gap-px">
				{WEEKDAYS.map((label) => (
					<div key={label} className="px-2 py-1 text-xs text-fg-faint">
						{label}
					</div>
				))}
			</div>

			<div
				className={cn(
					"grid min-h-0 flex-1 grid-cols-7 gap-px overflow-y-auto rounded-lg border border-hairline bg-hairline",
					mode === "month" ? "grid-rows-6" : "grid-rows-1",
				)}
			>
				{cells.map((cell) => (
					<DayCell
						key={cell.iso}
						cell={cell}
						isToday={cell.iso === today}
						entries={chips.get(cell.iso) ?? []}
						titleOf={titleOf}
						onOpen={openProject}
					/>
				))}
			</div>
		</div>
	);
}

function DayCell({
	cell,
	isToday,
	entries,
	titleOf,
	onOpen,
}: {
	cell: CalendarCell;
	isToday: boolean;
	entries: ScheduleEntry[];
	titleOf: (slug: string) => string;
	onOpen: (slug: string) => void;
}) {
	return (
		<div
			className={cn(
				"grain flex min-h-20 flex-col gap-1 bg-surface p-1.5",
				!cell.inMonth && "bg-surface/50 text-fg-faint",
			)}
		>
			<span
				className={cn(
					"flex size-5 items-center justify-center self-start rounded-full text-xs tabular-nums",
					isToday ? "bg-ember font-medium text-on-ember" : "text-fg-muted",
				)}
			>
				{cell.day}
			</span>
			<div className="flex flex-col gap-1">
				{entries.map((entry) => (
					<button
						key={entry.id}
						type="button"
						onClick={() => onOpen(entry.project_slug)}
						className="text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember"
					>
						<Badge
							variant="secondary"
							className="w-full justify-start gap-1 font-normal"
						>
							{entry.kind === "publish" ? <UploadSimpleIcon /> : <CameraIcon />}
							<span className="truncate">{titleOf(entry.project_slug)}</span>
						</Badge>
					</button>
				))}
			</div>
		</div>
	);
}
