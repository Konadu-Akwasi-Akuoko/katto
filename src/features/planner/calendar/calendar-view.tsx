import {
	ArrowRightIcon,
	CameraIcon,
	CaretLeftIcon,
	CaretRightIcon,
	CheckIcon,
	LightbulbIcon,
	PlusIcon,
	UploadSimpleIcon,
} from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { type ComponentType, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "@/components/ui/popover";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import type {
	CalendarCategory,
	CalendarCell,
	CalendarFilters,
	CalendarMode,
} from "@/features/planner/model/calendar";
import {
	ALL_PHASES,
	addDaysIso,
	addMonthsIso,
	applyCalendarFilters,
	markersByDate,
	monthGrid,
	periodLabel,
	weekRow,
} from "@/features/planner/model/calendar";
import { statusAppearance } from "@/lib/appearance";
import {
	type CalendarMarker,
	calendarKeys,
	listCalendar,
} from "@/lib/ipc/calendar";
import type { Project } from "@/lib/ipc/projects";
import { listProjects, projectsKeys } from "@/lib/ipc/projects";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";
import { PinPopover } from "./pin-popover";

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const ALL_PROJECTS = "__all__";

/** Legend/color metadata per category. Class names are literals so Tailwind keeps them. */
const CATEGORIES: ReadonlyArray<{
	key: CalendarCategory;
	label: string;
	Icon: ComponentType<{ className?: string }>;
	text: string;
	tint: string;
	dot: string;
}> = [
	{
		key: "shoot",
		label: "Shoot",
		Icon: CameraIcon,
		text: "text-cal-shoot",
		tint: "bg-cal-shoot/15",
		dot: "bg-cal-shoot",
	},
	{
		key: "publish",
		label: "Publish",
		Icon: UploadSimpleIcon,
		text: "text-cal-publish",
		tint: "bg-cal-publish/15",
		dot: "bg-cal-publish",
	},
	{
		key: "backlog",
		label: "Backlog",
		Icon: LightbulbIcon,
		text: "text-cal-backlog",
		tint: "bg-cal-backlog/15",
		dot: "bg-cal-backlog",
	},
	{
		key: "phase",
		label: "Phase",
		Icon: ArrowRightIcon,
		text: "text-cal-phase",
		tint: "bg-cal-phase/15",
		dot: "bg-cal-phase",
	},
];
const CATEGORY_BY_KIND = Object.fromEntries(
	CATEGORIES.map((c) => [c.key, c]),
) as Record<CalendarCategory, (typeof CATEGORIES)[number]>;

const DEFAULT_FILTERS: CalendarFilters = {
	categories: { shoot: true, publish: true, backlog: true, phase: true },
	phases: ALL_PHASES,
	project: null,
};

/** Today as a local calendar date (`YYYY-MM-DD`), matching the schedule's dates. */
function todayIso(): string {
	const now = new Date();
	const pad = (n: number) => String(n).padStart(2, "0");
	return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/** A marker's day-chip label — phase markers lead with their destination phase. */
function markerLabel(m: CalendarMarker): string {
	if (m.kind === "phase") return `${statusAppearance(m.to).label} · ${m.title}`;
	return m.title;
}

export function CalendarView() {
	const openPeek = useUiStore((s) => s.openPeek);
	const openIdea = useUiStore((s) => s.openIdea);
	const [mode, setMode] = useState<CalendarMode>("month");
	const [anchor, setAnchor] = useState(todayIso);
	const [filters, setFilters] = useState<CalendarFilters>(DEFAULT_FILTERS);
	const today = todayIso();

	const weeks = useMemo<CalendarCell[][]>(() => {
		if (mode === "week") return [weekRow(anchor)];
		const [year, month] = anchor.split("-").map(Number);
		return monthGrid(year ?? 0, (month ?? 1) - 1);
	}, [mode, anchor]);

	const cells = useMemo(() => weeks.flat(), [weeks]);
	const from = cells[0]?.iso ?? anchor;
	const to = cells[cells.length - 1]?.iso ?? anchor;

	const { data: markers } = useQuery({
		queryKey: calendarKeys.range(from, to),
		queryFn: () => listCalendar(from, to),
	});
	const { data: projects } = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});

	const byDate = useMemo(
		() => markersByDate(applyCalendarFilters(markers ?? [], filters)),
		[markers, filters],
	);

	function step(delta: number) {
		setAnchor((current) =>
			mode === "week"
				? addDaysIso(current, delta * 7)
				: addMonthsIso(current, delta),
		);
	}

	function toggleCategory(key: CalendarCategory) {
		setFilters((f) => ({
			...f,
			categories: { ...f.categories, [key]: !f.categories[key] },
		}));
	}

	function togglePhase(phase: string) {
		setFilters((f) => ({
			...f,
			phases: f.phases.includes(phase)
				? f.phases.filter((p) => p !== phase)
				: [...f.phases, phase],
		}));
	}

	function onMarkerClick(m: CalendarMarker) {
		if (m.kind === "backlog") openIdea(m.idea_id);
		else openPeek(m.project_slug);
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

			<div className="flex flex-wrap items-center gap-2">
				{CATEGORIES.map((c) => {
					const on = filters.categories[c.key];
					return (
						<button
							key={c.key}
							type="button"
							aria-pressed={on}
							aria-label={c.label}
							onClick={() => toggleCategory(c.key)}
							className={cn(
								"inline-flex h-7 cursor-default items-center gap-1.5 rounded-md border px-2.5 text-xs transition-opacity",
								on ? "text-fg" : "text-fg-faint opacity-45",
							)}
						>
							<span className={cn("size-2 rounded-full", c.dot)} />
							{c.label}
						</button>
					);
				})}

				<Popover>
					<PopoverTrigger asChild>
						<Button variant="secondary" size="sm">
							Phases · {filters.phases.length}
						</Button>
					</PopoverTrigger>
					<PopoverContent className="w-44 p-1">
						{ALL_PHASES.map((p) => {
							const checked = filters.phases.includes(p);
							return (
								<button
									key={p}
									type="button"
									aria-pressed={checked}
									onClick={() => togglePhase(p)}
									className="flex w-full cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-surface-2"
								>
									<span
										className={cn(
											"flex size-4 items-center justify-center rounded border",
											checked
												? "border-ember bg-ember text-on-ember"
												: "border-border",
										)}
									>
										{checked && <CheckIcon className="size-3" />}
									</span>
									{statusAppearance(p).label}
								</button>
							);
						})}
					</PopoverContent>
				</Popover>

				<Select
					value={filters.project ?? ALL_PROJECTS}
					onValueChange={(v) =>
						setFilters((f) => ({
							...f,
							project: v === ALL_PROJECTS ? null : v,
						}))
					}
				>
					<SelectTrigger size="sm" aria-label="Project" className="w-44">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						<SelectItem value={ALL_PROJECTS}>All projects</SelectItem>
						{projects?.map((p) => (
							<SelectItem key={p.slug} value={p.slug}>
								{p.title}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
			</div>

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
						markers={byDate.get(cell.iso) ?? []}
						projects={projects ?? []}
						onMarkerClick={onMarkerClick}
					/>
				))}
			</div>
		</div>
	);
}

function MarkerBadge({ m }: { m: CalendarMarker }) {
	const c = CATEGORY_BY_KIND[m.kind];
	return (
		<Badge
			variant="ghost"
			className={cn("w-full justify-start gap-1 font-normal", c.text, c.tint)}
		>
			<c.Icon className="size-3" />
			<span className="truncate">{markerLabel(m)}</span>
		</Badge>
	);
}

function DayCell({
	cell,
	isToday,
	markers,
	projects,
	onMarkerClick,
}: {
	cell: CalendarCell;
	isToday: boolean;
	markers: CalendarMarker[];
	projects: Project[];
	onMarkerClick: (m: CalendarMarker) => void;
}) {
	return (
		<div
			className={cn(
				"group grain relative flex min-h-20 flex-col gap-1 bg-surface p-1.5",
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

			<PinPopover ctx={{ mode: "add", date: cell.iso }} projects={projects}>
				<button
					type="button"
					aria-label={`Add pin on ${cell.iso}`}
					className="absolute top-1.5 right-1.5 flex size-5 cursor-default items-center justify-center rounded-md text-fg-faint opacity-0 transition-opacity hover:text-fg group-hover:opacity-100 focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ember focus-visible:outline-none"
				>
					<PlusIcon className="size-3.5" />
				</button>
			</PinPopover>

			<div className="flex flex-col gap-1">
				{markers.map((m, i) => {
					const key =
						m.kind === "backlog"
							? `b-${m.idea_id}`
							: `${m.kind}-${m.project_slug}-${i}`;
					// Shoot/publish are the editable planned layer — their chip opens the
					// pin editor. Backlog/phase are read-only history — they jump to detail.
					if (m.kind === "shoot" || m.kind === "publish") {
						return (
							<PinPopover
								key={key}
								projects={projects}
								ctx={{
									mode: "edit",
									date: m.date,
									projectSlug: m.project_slug,
									kind: m.kind,
									note: m.note,
								}}
							>
								<button
									type="button"
									className="text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember"
								>
									<MarkerBadge m={m} />
								</button>
							</PinPopover>
						);
					}
					return (
						<button
							key={key}
							type="button"
							onClick={() => onMarkerClick(m)}
							className="text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember"
						>
							<MarkerBadge m={m} />
						</button>
					);
				})}
			</div>
		</div>
	);
}
