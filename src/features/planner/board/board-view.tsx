import type { DragEndEvent } from "@dnd-kit/core";
import {
	closestCorners,
	DndContext,
	KeyboardSensor,
	PointerSensor,
	useDraggable,
	useDroppable,
	useSensor,
	useSensors,
} from "@dnd-kit/core";
import { CSS } from "@dnd-kit/utilities";
import type { Icon } from "@phosphor-icons/react";
import {
	ArrowSquareOutIcon,
	ArrowsLeftRightIcon,
	CameraIcon,
	CheckCircleIcon,
	FlagIcon,
	LightbulbIcon,
	ScissorsIcon,
	UploadSimpleIcon,
	VideoCameraIcon,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Badge } from "@/components/ui/badge";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuRadioGroup,
	ContextMenuRadioItem,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { groupByStatus } from "@/features/planner/model/board";
import { useDriveStatus } from "@/hooks/use-drive-status";
import {
	isPriorityLevel,
	PRIORITY_LEVELS,
	priorityAppearance,
	statusAppearance,
} from "@/lib/appearance";
import { formatShortDate } from "@/lib/date";
import type { PriorityLevel, Project } from "@/lib/ipc/projects";
import {
	listProjects,
	projectsKeys,
	setProjectPriority,
	setProjectStatus,
} from "@/lib/ipc/projects";
import type { ProjectStatus } from "@/lib/project-status";
import { isProjectStatus, PROJECT_STATUSES } from "@/lib/project-status";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";

const COLUMN_ICONS: Record<ProjectStatus, Icon> = {
	idea: LightbulbIcon,
	shooting: VideoCameraIcon,
	editing: ScissorsIcon,
	published: CheckCircleIcon,
};

/** Menu-only labels: "None" is a real radio value here, but priorityAppearance
 *  returns null for it because a card renders no chrome for an unset priority.
 *  Record<PriorityLevel, string> is exhaustive — a new Rust variant breaks it. */
const PRIORITY_MENU_LABELS: Record<PriorityLevel, string> = {
	none: "None",
	low: "Low",
	medium: "Medium",
	high: "High",
};

export function BoardView() {
	const queryClient = useQueryClient();
	const { data: projects, isError } = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});

	const move = useMutation({
		mutationFn: ({ slug, status }: { slug: string; status: ProjectStatus }) =>
			setProjectStatus(slug, status),
		onMutate: async ({ slug, status }) => {
			await queryClient.cancelQueries({ queryKey: projectsKeys.all });
			const previous = queryClient.getQueryData<Project[]>(projectsKeys.all);
			queryClient.setQueryData<Project[]>(projectsKeys.all, (old) =>
				old?.map((project) =>
					project.slug === slug ? { ...project, status } : project,
				),
			);
			return { previous };
		},
		onError: (_error, _variables, context) => {
			if (context?.previous) {
				queryClient.setQueryData(projectsKeys.all, context.previous);
			}
		},
		onSettled: () => {
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
		},
	});

	// Priority and status are folder-truth, so both writes sit behind the
	// engine's require_mounted guard. Disabling here is what turns that guard
	// into something the owner can see before clicking.
	const mounted = useDriveStatus().data?.mounted ?? false;

	const sensors = useSensors(
		useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
		useSensor(KeyboardSensor),
	);

	function onDragEnd(event: DragEndEvent) {
		const target = event.over?.id;
		if (typeof target !== "string" || !isProjectStatus(target)) return;
		const slug = String(event.active.id);
		const current = projects?.find((project) => project.slug === slug);
		if (!current || current.status === target) return;
		move.mutate({ slug, status: target });
	}

	// Only a cold failed load earns the error screen. Query keeps `data` and
	// flips to `error` on any background refetch failure (refetch-on-focus,
	// the drop mutation's invalidate) — blanking a good board there would be
	// worse than the toast the QueryCache already raises.
	if (isError && projects === undefined) {
		return (
			<div className="flex h-full flex-col items-center justify-center gap-1 text-sm">
				<p className="text-fg">Couldn't load your projects.</p>
				<p className="text-fg-muted">
					Check that the studio drive is mounted, then rescan from Projects.
				</p>
			</div>
		);
	}

	const groups = groupByStatus(projects ?? []);

	return (
		<DndContext
			sensors={sensors}
			collisionDetection={closestCorners}
			onDragEnd={onDragEnd}
		>
			<div className="flex h-full min-h-0 gap-3 overflow-x-auto pb-2">
				{PROJECT_STATUSES.map((column) => (
					<Column
						key={column}
						column={column}
						projects={groups[column]}
						mounted={mounted}
						onMove={(slug, status) => move.mutate({ slug, status })}
					/>
				))}
			</div>
		</DndContext>
	);
}

function Column({
	column,
	projects,
	mounted,
	onMove,
}: {
	column: ProjectStatus;
	projects: Project[];
	mounted: boolean;
	onMove: (slug: string, status: ProjectStatus) => void;
}) {
	const { setNodeRef, isOver } = useDroppable({ id: column });
	const { label, fg } = statusAppearance(column);
	const ColumnIcon = COLUMN_ICONS[column];

	return (
		<section className="flex w-64 shrink-0 flex-col gap-2">
			<header className="flex items-center justify-between px-1">
				<h2 className={cn("flex items-center gap-1.5 text-sm font-medium", fg)}>
					<ColumnIcon className="size-4" />
					{label}
				</h2>
				<span className="tabular-nums text-xs text-fg-faint">
					{projects.length}
				</span>
			</header>
			<div
				ref={setNodeRef}
				data-over={isOver}
				className={cn(
					"flex min-h-24 flex-1 flex-col gap-2 rounded-lg border border-dashed border-transparent p-1 transition-colors duration-fast",
					isOver && "border-ember/40 bg-surface-2/40",
				)}
			>
				{projects.map((project) => (
					<Card
						key={project.slug}
						project={project}
						mounted={mounted}
						onMove={onMove}
					/>
				))}
			</div>
		</section>
	);
}

function Card({
	project,
	mounted,
	onMove,
}: {
	project: Project;
	mounted: boolean;
	onMove: (slug: string, status: ProjectStatus) => void;
}) {
	const queryClient = useQueryClient();
	const { attributes, listeners, setNodeRef, transform, isDragging } =
		useDraggable({ id: project.slug });
	const openPeek = useUiStore((s) => s.openPeek);
	const priority = priorityAppearance(project.priority);

	const setPriority = useMutation({
		mutationFn: (level: PriorityLevel) =>
			setProjectPriority(project.slug, level),
		onSettled: () => {
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
		},
	});

	return (
		<ContextMenu>
			<ContextMenuTrigger asChild>
				{/* biome-ignore lint/a11y/noStaticElementInteractions: {...attributes} supplies role="button" + tabIndex={0} at runtime (useDraggable's defaultRole); Biome can't see through the spread */}
				{/* biome-ignore lint/a11y/useKeyWithClickEvents: satisfying this literally would replace the KeyboardSensor's onKeyDown activator (listeners already carries it) and kill keyboard drag. The click action reaches the keyboard through the context menu's Open item instead — the menu opens on Shift+F10 / the Menu key. */}
				<div
					ref={setNodeRef}
					{...listeners}
					{...attributes}
					onClick={() => openPeek(project.slug)}
					style={{ transform: CSS.Translate.toString(transform) }}
					className={cn(
						"grain flex cursor-default touch-none flex-col overflow-hidden rounded-lg border bg-surface",
						"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember",
						isDragging && "opacity-50",
					)}
				>
					{priority ? (
						<div
							className={cn(
								"flex w-fit items-center gap-1 rounded-br-lg px-2 py-0.5",
								// The one sanctioned uppercase-letterspaced label in the app
								// (spec §Design-rule exception, board card only). Scale steps
								// only — no arbitrary type values anywhere in this repo.
								"text-xs uppercase tracking-wider",
								priority.tint,
								priority.fg,
							)}
						>
							<FlagIcon className="size-3" />
							{priority.label}
						</div>
					) : null}
					<div className="flex flex-col gap-1.5 p-3 pt-2.5">
						<span className="text-sm text-fg">{project.title}</span>
						<DateChips project={project} />
					</div>
				</div>
			</ContextMenuTrigger>

			<ContextMenuContent>
				<ContextMenuItem onSelect={() => openPeek(project.slug)}>
					<ArrowSquareOutIcon />
					Open
				</ContextMenuItem>

				<ContextMenuSub>
					<ContextMenuSubTrigger disabled={!mounted}>
						<FlagIcon />
						Set priority
					</ContextMenuSubTrigger>
					<ContextMenuSubContent>
						<ContextMenuRadioGroup
							value={project.priority}
							onValueChange={(value) => {
								if (isPriorityLevel(value)) setPriority.mutate(value);
							}}
						>
							{PRIORITY_LEVELS.map((level) => (
								<ContextMenuRadioItem key={level} value={level}>
									<Swatch
										fg={priorityAppearance(level)?.fg ?? "text-fg-faint"}
									/>
									{PRIORITY_MENU_LABELS[level]}
								</ContextMenuRadioItem>
							))}
						</ContextMenuRadioGroup>
					</ContextMenuSubContent>
				</ContextMenuSub>

				<ContextMenuSub>
					<ContextMenuSubTrigger disabled={!mounted}>
						<ArrowsLeftRightIcon />
						Move to
					</ContextMenuSubTrigger>
					<ContextMenuSubContent>
						<ContextMenuRadioGroup
							value={project.status}
							onValueChange={(value) => {
								if (isProjectStatus(value)) onMove(project.slug, value);
							}}
						>
							{PROJECT_STATUSES.map((status) => (
								<ContextMenuRadioItem
									key={status}
									value={status}
									disabled={status === project.status}
								>
									<Swatch fg={statusAppearance(status).fg} />
									{statusAppearance(status).label}
								</ContextMenuRadioItem>
							))}
						</ContextMenuRadioGroup>
					</ContextMenuSubContent>
				</ContextMenuSub>
			</ContextMenuContent>
		</ContextMenu>
	);
}

/**
 * The menu's colour cue. A submenu of four priorities has no other way to tell
 * them apart, which is why the spec asks for swatches here; `Appearance` carries
 * no dot class, so the colour rides in on `fg` and the fill picks it up.
 */
function Swatch({ fg }: { fg: string }) {
	return <span className={cn("size-2 rounded-full bg-current", fg)} />;
}

function DateChips({ project }: { project: Project }) {
	if (!project.shoot_date && !project.publish_date) return null;

	return (
		<div className="flex flex-wrap gap-1">
			{project.shoot_date ? (
				<Badge variant="secondary" className="gap-1 font-normal">
					<CameraIcon />
					{formatShortDate(project.shoot_date)}
				</Badge>
			) : null}
			{project.publish_date ? (
				<Badge variant="secondary" className="gap-1 font-normal">
					<UploadSimpleIcon />
					{formatShortDate(project.publish_date)}
				</Badge>
			) : null}
		</div>
	);
}
