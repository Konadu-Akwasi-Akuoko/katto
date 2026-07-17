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
	TrashIcon,
	UploadSimpleIcon,
	VideoCameraIcon,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuRadioGroup,
	ContextMenuRadioItem,
	ContextMenuSeparator,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { groupByStatus } from "@/features/planner/model/board";
import { useDriveStatus } from "@/hooks/use-drive-status";
import {
	isPriorityLevel,
	PRIORITY_LEVELS,
	PRIORITY_MENU_LABELS,
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
	trashProject,
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

	const [pendingDelete, setPendingDelete] = useState<Project | null>(null);
	const closePeek = useUiStore((s) => s.closePeek);

	// Deliberately not optimistic. A Trash that the OS refuses leaves the folder
	// on disk, so removing the card first would have the board lying about what
	// is there; MutationCache.onError already surfaces the failure.
	const trash = useMutation({
		mutationFn: (slug: string) => trashProject(slug),
		onSuccess: (_data, slug) => {
			// The peek is very likely open on the card that was just right-clicked;
			// leaving it would refetch a project that no longer exists and report a
			// load failure for a deliberate delete.
			if (useUiStore.getState().peekSlug === slug) closePeek();
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
		},
		onSettled: () => setPendingDelete(null),
	});

	// Priority, status and delete are folder-truth, so all three writes sit
	// behind the engine's require_mounted guard. Disabling here is what turns
	// that guard into something the owner can see before clicking.
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
		<>
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
							onRequestDelete={setPendingDelete}
						/>
					))}
				</div>
			</DndContext>

			{/* Controlled, and a sibling of every menu tree: a DialogTrigger inside
			    the ContextMenuItem would unmount with the menu on select. The
			    content is rendered only while a project is pending rather than
			    reading it optionally: Radix's Presence holds closed content for
			    the 200ms exit animation, so `pendingDelete?.title` would flash an
			    empty name on every close. Unmounting trades that fade away. */}
			<Dialog
				open={pendingDelete !== null}
				onOpenChange={(open) => {
					if (!open) setPendingDelete(null);
				}}
			>
				{pendingDelete ? (
					<DialogContent>
						<DialogHeader>
							<DialogTitle>
								Move “{pendingDelete.title}” to the Trash?
							</DialogTitle>
							<DialogDescription>
								The project folder goes to the Trash and its row is removed. Put
								it back from the Trash and katto picks it up on the next rescan.
							</DialogDescription>
						</DialogHeader>
						<DialogFooter>
							<Button
								variant="secondary"
								onClick={() => setPendingDelete(null)}
							>
								Cancel
							</Button>
							<Button
								variant="destructive"
								disabled={trash.isPending}
								onClick={() => trash.mutate(pendingDelete.slug)}
							>
								<TrashIcon />
								Move to Trash
							</Button>
						</DialogFooter>
					</DialogContent>
				) : null}
			</Dialog>
		</>
	);
}

function Column({
	column,
	projects,
	mounted,
	onMove,
	onRequestDelete,
}: {
	column: ProjectStatus;
	projects: Project[];
	mounted: boolean;
	onMove: (slug: string, status: ProjectStatus) => void;
	onRequestDelete: (project: Project) => void;
}) {
	const { setNodeRef, isOver } = useDroppable({ id: column });
	const { label, fg } = statusAppearance(column);
	const ColumnIcon = COLUMN_ICONS[column];

	return (
		<section className="flex w-64 shrink-0 flex-col gap-2">
			<header className="flex items-center justify-between px-1">
				{/* The status colour rides on the icon, not the label. A status hue on
				    body-size text over the plain --bg lands at ~3.2:1 for editing and
				    ~3.5:1 for shooting — under the 4.5:1 text floor. A 16px icon is a
				    non-text graphic (3:1 floor), which both clear, so the state stays
				    encoded once in colour + glyph and the label keeps its ~12:1. */}
				<h2 className="flex items-center gap-1.5 text-sm font-medium text-fg">
					<ColumnIcon className={cn("size-4", fg)} />
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
						onRequestDelete={onRequestDelete}
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
	onRequestDelete,
}: {
	project: Project;
	mounted: boolean;
	onMove: (slug: string, status: ProjectStatus) => void;
	onRequestDelete: (project: Project) => void;
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
				{/* biome-ignore lint/a11y/useKeyWithClickEvents: satisfying this literally would replace the KeyboardSensor's onKeyDown activator (listeners already carries it) and kill keyboard drag. Task 9's context-menu Open item was meant to close this, and does NOT: Radix's trigger opens on contextmenu/pointerdown only, with no keydown of its own, so it inherits the platform's keyboard route to a context menu — and macOS has none (no Menu key, no Shift+F10). The click action is still not keyboard-reachable. Fixing it needs a route that isn't the card's own onKeyDown: a menubar/palette command, or a keyboard shortcut bound above the card. */}
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

				<ContextMenuSeparator />

				<ContextMenuItem
					variant="destructive"
					disabled={!mounted}
					onSelect={() => onRequestDelete(project)}
				>
					<TrashIcon />
					Delete
				</ContextMenuItem>
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
