import {
	ArrowSquareOutIcon,
	CameraIcon,
	ClockIcon,
	FolderIcon,
	UploadSimpleIcon,
} from "@phosphor-icons/react";
import {
	skipToken,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { PriorityChip } from "@/components/ui/priority-chip";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	Sheet,
	SheetContent,
	SheetFooter,
	SheetHeader,
	SheetTitle,
} from "@/components/ui/sheet";
import { StatusChip } from "@/components/ui/status-chip";
import { useDriveStatus } from "@/hooks/use-drive-status";
import {
	isPriorityLevel,
	PRIORITY_LEVELS,
	PRIORITY_MENU_LABELS,
	priorityAppearance,
} from "@/lib/appearance";
import { formatShortDate } from "@/lib/date";
import type {
	PriorityLevel,
	Project,
	ProjectDetail,
	ProjectKind,
} from "@/lib/ipc/projects";
import {
	getProject,
	projectsKeys,
	revealProjectFolder,
	setProjectKind,
	setProjectPriority,
} from "@/lib/ipc/projects";
import { isProjectKind, KIND_LABELS, PROJECT_KINDS } from "@/lib/kind";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";

/**
 * The shared project peek: a right-side drawer with a scan of one project
 * (status, dates, folder freshness) and a jump to its full detail page. Mounted
 * once at the shell; opened by setting `peekSlug` (board card, calendar event).
 */
export function ProjectPeek() {
	const peekSlug = useUiStore((s) => s.peekSlug);
	const closePeek = useUiStore((s) => s.closePeek);
	const openProject = useUiStore((s) => s.openProject);

	const { data, isError } = useQuery({
		queryKey: projectsKeys.detail(peekSlug ?? ""),
		queryFn: peekSlug === null ? skipToken : () => getProject(peekSlug),
		enabled: peekSlug !== null,
	});

	const reveal = useMutation({
		mutationFn: (slug: string) => revealProjectFolder(slug),
	});

	return (
		<Sheet
			open={peekSlug !== null}
			onOpenChange={(open) => {
				if (!open) closePeek();
			}}
		>
			<SheetContent aria-describedby={undefined}>
				{peekSlug === null ? null : isError ? (
					<SheetHeader>
						<SheetTitle>Couldn't load this project</SheetTitle>
						<p className="text-sm text-fg-muted">
							The studio drive may be unmounted, or the folder may have moved.
							Rescan from Projects to reconcile.
						</p>
					</SheetHeader>
				) : !data ? (
					<SheetHeader>
						<SheetTitle>Loading…</SheetTitle>
					</SheetHeader>
				) : (
					<PeekBody
						detail={data}
						onOpenDetail={() => {
							openProject(data.project.slug);
							closePeek();
						}}
						onReveal={() => reveal.mutate(data.project.slug)}
					/>
				)}
			</SheetContent>
		</Sheet>
	);
}

function PeekBody({
	detail,
	onOpenDetail,
	onReveal,
}: {
	detail: ProjectDetail;
	onOpenDetail: () => void;
	onReveal: () => void;
}) {
	const { project, manifest_error, freshness } = detail;

	return (
		<>
			<SheetHeader>
				<SheetTitle>{project.title}</SheetTitle>
				<div className="flex flex-wrap items-center gap-1.5">
					<StatusChip status={project.status} />
					<PriorityControl project={project} />
					<KindControl project={project} />
				</div>
			</SheetHeader>

			{manifest_error ? (
				<p
					className="rounded-md border border-failed/40 bg-failed/10 p-2 text-sm text-failed"
					style={{ backgroundImage: "none" }}
				>
					Invalid manifest: {manifest_error}
				</p>
			) : (
				<>
					<dl className="flex flex-col gap-2 text-sm">
						<MetaRow
							icon={<CameraIcon />}
							label="Shoot"
							value={
								project.shoot_date ? formatShortDate(project.shoot_date) : "—"
							}
						/>
						<MetaRow
							icon={<UploadSimpleIcon />}
							label="Publish"
							value={
								project.publish_date
									? formatShortDate(project.publish_date)
									: "—"
							}
						/>
					</dl>

					{freshness.length > 0 ? (
						<div className="flex flex-col gap-1">
							{freshness.map((folder) => (
								<div
									key={folder.subfolder}
									className="flex items-center gap-2 text-xs text-fg-muted [&_svg]:size-4"
								>
									<FolderIcon />
									<span className="flex-1 truncate text-fg">
										{folder.subfolder}
									</span>
									<span className="tabular-nums">{folder.file_count}</span>
									<ClockIcon />
									<span className="tabular-nums">
										{folder.latest_mtime
											? formatShortDate(folder.latest_mtime)
											: "—"}
									</span>
								</div>
							))}
						</div>
					) : null}
				</>
			)}

			<SheetFooter>
				<Button onClick={onOpenDetail}>
					<ArrowSquareOutIcon />
					Open full detail
				</Button>
				<Button variant="secondary" onClick={onReveal}>
					<FolderIcon />
					Reveal in Finder
				</Button>
			</SheetFooter>
		</>
	);
}

/**
 * The peek's priority axis, readable and writable. Phase 4 opens this drawer
 * from a calendar event — a surface with no card and no context menu — so the
 * chip alone would be a dead end there. `PriorityChip` stays pure display; this
 * wraps it. Disabled on an unmounted root: priority is folder-truth, so the
 * write sits behind the engine's require_mounted guard like every other one.
 */
function PriorityControl({ project }: { project: Project }) {
	const queryClient = useQueryClient();
	const mounted = useDriveStatus().data?.mounted ?? false;

	const setPriority = useMutation({
		mutationFn: (level: PriorityLevel) =>
			setProjectPriority(project.slug, level),
		onSettled: () => {
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
			void queryClient.invalidateQueries({
				queryKey: projectsKeys.detail(project.slug),
			});
		},
	});

	return (
		<Select
			value={project.priority}
			disabled={!mounted}
			onValueChange={(value) => {
				if (isPriorityLevel(value)) setPriority.mutate(value);
			}}
		>
			{/* The chip is the control's whole face: the primitive's border, padding
			    and shadow would box a chip that already reads as one thing, and a
			    shadow on an inline control isn't the floating layer --shadow is for. */}
			<SelectTrigger
				size="sm"
				aria-label="Priority"
				className="border-none px-0 shadow-none"
			>
				<SelectValue>
					{priorityAppearance(project.priority) ? (
						<PriorityChip priority={project.priority} />
					) : (
						<span className="text-sm text-fg-faint">Set priority</span>
					)}
				</SelectValue>
			</SelectTrigger>
			<SelectContent>
				{PRIORITY_LEVELS.map((level) => (
					<SelectItem key={level} value={level}>
						{/* The same colour cue the context menu's radio set carries — the
						    two are one control in two places and must not drift. */}
						<span
							className={cn(
								"size-2 rounded-full bg-current",
								priorityAppearance(level)?.fg ?? "text-fg-faint",
							)}
						/>
						{PRIORITY_MENU_LABELS[level]}
					</SelectItem>
				))}
			</SelectContent>
		</Select>
	);
}

/**
 * The peek's kind axis, readable and writable. Mirrors `PriorityControl`: the
 * chip is the whole trigger face, and the write sits behind the engine's
 * require_mounted guard, so it disables on an unmounted root. Kind always reads a
 * value — an unset project shows "Unsorted" rather than an empty control.
 */
function KindControl({ project }: { project: Project }) {
	const queryClient = useQueryClient();
	const mounted = useDriveStatus().data?.mounted ?? false;

	const setKind = useMutation({
		mutationFn: (kind: ProjectKind) => setProjectKind(project.slug, kind),
		onSettled: () => {
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
			void queryClient.invalidateQueries({
				queryKey: projectsKeys.detail(project.slug),
			});
		},
	});

	return (
		<Select
			value={isProjectKind(project.kind) ? project.kind : undefined}
			disabled={!mounted}
			onValueChange={(value) => {
				if (isProjectKind(value)) setKind.mutate(value);
			}}
		>
			<SelectTrigger
				size="sm"
				aria-label="Kind"
				className="border-none px-0 shadow-none"
			>
				<SelectValue>
					<span className="inline-flex h-[19px] items-center rounded-md bg-surface-2 px-2 text-xs text-fg-muted">
						{KIND_LABELS[project.kind] ?? project.kind}
					</span>
				</SelectValue>
			</SelectTrigger>
			<SelectContent>
				{PROJECT_KINDS.map((kind) => (
					<SelectItem key={kind} value={kind}>
						{KIND_LABELS[kind]}
					</SelectItem>
				))}
			</SelectContent>
		</Select>
	);
}

function MetaRow({
	icon,
	label,
	value,
}: {
	icon: ReactNode;
	label: string;
	value: string;
}) {
	return (
		<div className="flex items-center gap-2">
			<span className="text-fg-muted [&_svg]:size-4">{icon}</span>
			<dt className="w-16 text-fg-muted">{label}</dt>
			<dd className="tabular-nums text-fg">{value}</dd>
		</div>
	);
}
