import { DownloadSimpleIcon, XIcon } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	activeAssetProject,
	browserKeys,
	revealInProject,
	setActiveAssetProject,
} from "@/lib/ipc/browser";
import { listProjects, projectsKeys } from "@/lib/ipc/projects";
import { cn } from "@/lib/utils";
import type { DownloadRow } from "@/stores/downloads";
import { useDownloadsStore } from "@/stores/downloads";

function rowMeta(row: DownloadRow): string {
	switch (row.status) {
		case "filed":
			return `→ ${row.project}/${row.destRel}`;
		case "filing":
			return "filing…";
		case "needs-project":
			return "waiting for a project pick";
		case "fallback":
			return "saved to Downloads";
		case "failed":
			return "failed";
	}
}

function DownloadRowItem({ row }: { row: DownloadRow }) {
	const dismiss = useDownloadsStore((s) => s.dismiss);
	const setNeedsProject = useDownloadsStore((s) => s.setNeedsProject);
	const filed =
		row.status === "filed" &&
		row.project !== undefined &&
		row.destRel !== undefined;
	const reveal = useMutation({
		mutationFn: (target: { project: string; destRel: string }) =>
			revealInProject(target.project, target.destRel),
	});
	return (
		<div className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-surface-2">
			<div className="min-w-0 flex-1">
				<div
					className={cn(
						"truncate text-xs text-fg",
						row.status === "failed" && "text-fg-muted line-through",
					)}
				>
					{row.filename}
				</div>
				<div
					className={cn(
						"truncate text-[11px] text-fg-faint",
						row.status === "filed" && "font-mono",
					)}
				>
					{rowMeta(row)}
				</div>
			</div>
			{row.status === "needs-project" && (
				<button
					type="button"
					onClick={() =>
						setNeedsProject({ id: row.id, filename: row.filename })
					}
					className="shrink-0 rounded-md px-1.5 py-0.5 text-[11px] text-fg-muted hover:bg-surface hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
				>
					Choose project
				</button>
			)}
			{filed && (
				<button
					type="button"
					onClick={() => {
						if (row.project !== undefined && row.destRel !== undefined) {
							reveal.mutate({ project: row.project, destRel: row.destRel });
						}
					}}
					className="shrink-0 rounded-md px-1.5 py-0.5 text-[11px] text-fg-muted hover:bg-surface hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
				>
					Reveal
				</button>
			)}
			{row.status !== "filing" && (
				<button
					type="button"
					aria-label={`Dismiss ${row.filename}`}
					onClick={() => dismiss(row.id)}
					className="shrink-0 rounded-sm p-0.5 text-fg-faint hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
				>
					<XIcon className="size-3" />
				</button>
			)}
		</div>
	);
}

/** The toolbar control that opens the downloads panel. */
export function DownloadsButton({
	open,
	onToggle,
}: {
	open: boolean;
	onToggle: () => void;
}) {
	const rows = useDownloadsStore((s) => s.rows);
	return (
		<button
			type="button"
			aria-label="Downloads"
			aria-pressed={open}
			onClick={onToggle}
			className={cn(
				"relative flex size-7 shrink-0 items-center justify-center rounded-md text-fg-muted hover:bg-surface-2 hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2",
				open && "bg-surface-2 text-fg",
			)}
		>
			<DownloadSimpleIcon className="size-4" />
			{rows.some((r) => r.status === "filing") && (
				<span className="absolute top-1 right-1 size-1.5 rounded-full bg-ember" />
			)}
		</button>
	);
}

/**
 * Downloads live in a column beside the page, not floating over it: a native
 * child webview paints above every DOM layer, so an overlay could only be shown
 * by hiding the page, which blanked the pane. As a sibling of the content host
 * it takes width from it instead — the page reflows and stays live.
 */
export function DownloadsPanel() {
	const rows = useDownloadsStore((s) => s.rows);
	const queryClient = useQueryClient();
	const target = useQuery({
		queryKey: browserKeys.activeProject,
		queryFn: activeAssetProject,
	});
	const projects = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});
	const setTarget = useMutation({
		mutationFn: (slug: string) => setActiveAssetProject(slug),
		onSuccess: () =>
			queryClient.invalidateQueries({ queryKey: browserKeys.activeProject }),
	});

	return (
		<aside
			aria-label="Downloads"
			className="flex w-80 shrink-0 flex-col border-l bg-surface"
		>
			<div className="flex items-center justify-between gap-2 border-b px-2 py-1.5">
				<span className="text-xs text-fg-muted">Files to</span>
				<Select
					value={target.data ?? ""}
					onValueChange={(slug) => setTarget.mutate(slug)}
				>
					<SelectTrigger size="sm" className="h-6 w-44 text-xs">
						<SelectValue placeholder="no project" />
					</SelectTrigger>
					<SelectContent>
						{(projects.data ?? []).map((p) => (
							<SelectItem key={p.slug} value={p.slug}>
								{p.slug}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
			</div>
			<div className="min-h-0 flex-1 overflow-y-auto p-1">
				{rows.length === 0 ? (
					<p className="px-2 py-3 text-xs text-fg-faint">
						Nothing downloading. Grab something from a tab and it files itself.
					</p>
				) : (
					rows.map((row) => <DownloadRowItem key={row.id} row={row} />)
				)}
			</div>
		</aside>
	);
}
