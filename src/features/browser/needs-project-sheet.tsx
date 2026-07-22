import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
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
	SheetDescription,
	SheetFooter,
	SheetHeader,
	SheetTitle,
} from "@/components/ui/sheet";
import {
	browserKeys,
	fileParkedDownload,
	setActiveAssetProject,
} from "@/lib/ipc/browser";
import { listProjects, projectsKeys } from "@/lib/ipc/projects";
import { useDownloadsStore } from "@/stores/downloads";

/**
 * Opens when a download finished with no project to file into. Picking a
 * project files the parked download and becomes the filing target for the
 * next one; Discard leaves the bytes to the staging sweep.
 */
export function NeedsProjectSheet() {
	const needsProject = useDownloadsStore((s) => s.needsProject);
	const setNeedsProject = useDownloadsStore((s) => s.setNeedsProject);
	const dismiss = useDownloadsStore((s) => s.dismiss);
	const upsert = useDownloadsStore((s) => s.upsert);
	const queryClient = useQueryClient();
	const [slug, setSlug] = useState<string | null>(null);

	const projects = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});

	const file = useMutation({
		mutationFn: async (pick: { id: string; slug: string }) => {
			await fileParkedDownload(pick.id, pick.slug);
			await setActiveAssetProject(pick.slug);
		},
		onSuccess: (_data, pick) => {
			if (needsProject) {
				upsert({
					id: needsProject.id,
					filename: needsProject.filename,
					status: "filing",
					project: pick.slug,
				});
			}
			setNeedsProject(null);
			void queryClient.invalidateQueries({
				queryKey: browserKeys.activeProject,
			});
		},
	});

	function discard() {
		if (needsProject) dismiss(needsProject.id);
		setNeedsProject(null);
	}

	return (
		<Sheet
			open={needsProject !== null}
			onOpenChange={(open) => {
				if (!open) setNeedsProject(null);
			}}
		>
			<SheetContent>
				<SheetHeader>
					<SheetTitle>File this download</SheetTitle>
					<SheetDescription>
						No active project to file into. Pick one — katto remembers it for
						the next download.
					</SheetDescription>
				</SheetHeader>
				<div className="flex flex-col gap-3 px-4">
					<p className="truncate font-mono text-xs text-fg-muted">
						{needsProject?.filename}
					</p>
					<Select value={slug ?? ""} onValueChange={setSlug}>
						<SelectTrigger className="w-full">
							<SelectValue placeholder="Pick a project" />
						</SelectTrigger>
						<SelectContent>
							{(projects.data ?? []).map((p) => (
								<SelectItem key={p.slug} value={p.slug}>
									{p.title}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
				</div>
				<SheetFooter>
					<Button
						disabled={slug === null || file.isPending}
						onClick={() => {
							if (needsProject && slug !== null) {
								file.mutate({ id: needsProject.id, slug });
							}
						}}
					>
						{slug === null ? "File download" : `File to ${slug}`}
					</Button>
					<Button variant="secondary" onClick={discard}>
						Discard download
					</Button>
				</SheetFooter>
			</SheetContent>
		</Sheet>
	);
}
