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
import { CameraIcon, UploadSimpleIcon } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Badge } from "@/components/ui/badge";
import { groupByStatus } from "@/features/planner/model/board";
import { formatShortDate } from "@/lib/date";
import type { Project } from "@/lib/ipc/projects";
import {
	listProjects,
	projectsKeys,
	setProjectStatus,
} from "@/lib/ipc/projects";
import type { ProjectStatus } from "@/lib/project-status";
import { isProjectStatus, PROJECT_STATUSES } from "@/lib/project-status";
import { cn } from "@/lib/utils";

const COLUMN_LABELS: Record<ProjectStatus, string> = {
	idea: "Idea",
	shooting: "Shooting",
	editing: "Editing",
	published: "Published",
};

export function BoardView() {
	const queryClient = useQueryClient();
	const { data: projects } = useQuery({
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

	const groups = groupByStatus(projects ?? []);

	return (
		<DndContext
			sensors={sensors}
			collisionDetection={closestCorners}
			onDragEnd={onDragEnd}
		>
			<div className="flex h-full min-h-0 gap-3 overflow-x-auto pb-2">
				{PROJECT_STATUSES.map((column) => (
					<Column key={column} column={column} projects={groups[column]} />
				))}
			</div>
		</DndContext>
	);
}

function Column({
	column,
	projects,
}: {
	column: ProjectStatus;
	projects: Project[];
}) {
	const { setNodeRef, isOver } = useDroppable({ id: column });

	return (
		<section className="flex w-64 shrink-0 flex-col gap-2">
			<header className="flex items-baseline justify-between px-1">
				<h2 className="text-sm font-medium text-fg">{COLUMN_LABELS[column]}</h2>
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
					<Card key={project.slug} project={project} />
				))}
			</div>
		</section>
	);
}

function Card({ project }: { project: Project }) {
	const { attributes, listeners, setNodeRef, transform, isDragging } =
		useDraggable({ id: project.slug });

	return (
		<div
			ref={setNodeRef}
			{...listeners}
			{...attributes}
			style={{ transform: CSS.Translate.toString(transform) }}
			className={cn(
				"grain flex cursor-default touch-none flex-col gap-1.5 rounded-lg border bg-surface p-3",
				"focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember",
				isDragging && "opacity-50",
			)}
		>
			<span className="text-sm text-fg">{project.title}</span>
			{/* v1 hint is the shoot/publish chip; the latest-artifact hint waits on
			    the cheaper detail freshness data (Task 14). */}
			<DateChips project={project} />
		</div>
	);
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
