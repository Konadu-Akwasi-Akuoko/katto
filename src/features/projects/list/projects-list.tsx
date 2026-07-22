import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { StatusChip } from "@/components/ui/status-chip";
import { formatDate } from "@/features/projects/model/format";
import type { Project } from "@/lib/ipc/projects";
import { listProjects, projectsKeys } from "@/lib/ipc/projects";
import { listLatestThumbnails, thumbKeys } from "@/lib/ipc/thumbnails";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";

export function ProjectsList() {
	const openDetail = useUiStore((s) => s.setSelectedProjectSlug);
	const justPromotedSlug = useUiStore((s) => s.justPromotedSlug);
	const clearJustPromoted = useUiStore((s) => s.setJustPromoted);
	const { data: projects } = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});
	const { data: thumbs } = useQuery({
		queryKey: thumbKeys.all,
		queryFn: listLatestThumbnails,
	});
	const thumbBySlug = new Map(
		(thumbs ?? []).map((thumb) => [thumb.slug, thumb.path]),
	);

	return (
		<div className="flex flex-col gap-4 p-6">
			<h1 className="font-serif text-2xl">Projects</h1>
			{projects === undefined ? null : projects.length === 0 ? (
				<p className="text-sm text-fg-muted">
					No projects yet. Promote an idea — the folder, card, and schedule
					arrive together.
				</p>
			) : (
				<ul className="flex flex-col gap-2">
					{projects.map((project) => (
						<li key={project.slug}>
							<button
								type="button"
								onClick={() => openDetail(project.slug)}
								onAnimationEnd={(event) => {
									if (event.animationName === "promote-arrival") {
										clearJustPromoted(null);
									}
								}}
								className={cn(
									"grain flex w-full items-center gap-3 rounded-lg border bg-surface px-3 py-2 text-left transition-colors hover:bg-surface-2",
									justPromotedSlug === project.slug &&
										"animate-promote-arrival",
								)}
							>
								{thumbBySlug.has(project.slug) && (
									<img
										src={convertFileSrc(thumbBySlug.get(project.slug) ?? "")}
										alt=""
										className="h-7 w-12 shrink-0 rounded-sm border object-cover"
									/>
								)}
								<span className="min-w-0 flex-1 truncate text-sm text-fg">
									{project.title}
								</span>
								<ProjectDates project={project} />
								<StatusChip status={project.status} />
							</button>
						</li>
					))}
				</ul>
			)}
		</div>
	);
}

function ProjectDates({ project }: { project: Project }) {
	if (!project.shoot_date && !project.publish_date) return null;
	return (
		<div className="flex shrink-0 gap-3 text-xs text-fg-faint">
			{project.shoot_date ? (
				<span>
					Shoot{" "}
					<span className="font-mono text-fg-muted tabular-nums">
						{formatDate(project.shoot_date)}
					</span>
				</span>
			) : null}
			{project.publish_date ? (
				<span>
					Publish{" "}
					<span className="font-mono text-fg-muted tabular-nums">
						{formatDate(project.publish_date)}
					</span>
				</span>
			) : null}
		</div>
	);
}
