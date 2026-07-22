import { DownloadSimpleIcon, XIcon } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
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
			return `→ ${row.project} · ${row.destRel}`;
		case "filing":
			return "filing…";
		case "needs-project":
			return "choosing project…";
		case "fallback":
			return "saved to Downloads";
		case "failed":
			return "failed";
	}
}

function DownloadRowItem({ row }: { row: DownloadRow }) {
	const dismiss = useDownloadsStore((s) => s.dismiss);
	const reveal = useMutation({
		mutationFn: () => revealInProject(row.project ?? "", row.destRel ?? ""),
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
				<div className="truncate text-[11px] text-fg-faint">{rowMeta(row)}</div>
			</div>
			{row.status === "filed" && (
				<button
					type="button"
					onClick={() => reveal.mutate()}
					className="shrink-0 rounded-md px-1.5 py-0.5 text-[11px] text-fg-muted hover:bg-surface hover:text-fg"
				>
					Reveal
				</button>
			)}
			{row.status !== "filing" && (
				<button
					type="button"
					aria-label={`Dismiss ${row.filename}`}
					onClick={() => dismiss(row.id)}
					className="shrink-0 rounded-sm p-0.5 text-fg-faint hover:text-fg"
				>
					<XIcon className="size-3" />
				</button>
			)}
		</div>
	);
}

/**
 * Downloads live in a quiet popover: filename, muted meta line naming where
 * it filed, right-aligned actions. The header names the filing target and
 * offers the switcher.
 */
export function DownloadsPopover() {
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
		<Popover>
			<PopoverTrigger
				aria-label="Downloads"
				className="relative flex size-7 items-center justify-center rounded-md text-fg-muted hover:bg-surface-2 hover:text-fg"
			>
				<DownloadSimpleIcon className="size-4" />
				{rows.some((r) => r.status === "filing") && (
					<span className="absolute top-1 right-1 size-1.5 rounded-full bg-ember" />
				)}
			</PopoverTrigger>
			<PopoverContent className="w-88">
				<div className="flex items-center justify-between gap-2 px-2 py-1.5">
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
				<div className="border-t pt-1">
					{rows.length === 0 ? (
						<p className="px-2 py-3 text-xs text-fg-faint">
							Nothing downloading. Grab something from a tab and it files
							itself.
						</p>
					) : (
						rows.map((row) => <DownloadRowItem key={row.id} row={row} />)
					)}
				</div>
			</PopoverContent>
		</Popover>
	);
}
