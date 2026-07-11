import {
	ArrowSquareOutIcon,
	CameraIcon,
	ClockIcon,
	FolderIcon,
	UploadSimpleIcon,
} from "@phosphor-icons/react";
import { useMutation, useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import {
	Sheet,
	SheetContent,
	SheetFooter,
	SheetHeader,
	SheetTitle,
} from "@/components/ui/sheet";
import { statusAppearance } from "@/features/planner/model/appearance";
import { formatShortDate } from "@/lib/date";
import type { ProjectDetail } from "@/lib/ipc/projects";
import {
	getProject,
	projectsKeys,
	revealProjectFolder,
} from "@/lib/ipc/projects";
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

	const { data, isLoading } = useQuery({
		queryKey: projectsKeys.detail(peekSlug ?? ""),
		queryFn: () => getProject(peekSlug as string),
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
				{peekSlug === null || isLoading || !data ? (
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
	const status = statusAppearance(project.status);

	return (
		<>
			<SheetHeader>
				<SheetTitle>{project.title}</SheetTitle>
				<div className="flex items-center gap-1.5 text-sm text-fg-muted">
					<span className={cn("size-2 rounded-full", status.dot)} />
					{status.label}
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
