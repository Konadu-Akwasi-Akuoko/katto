import { WarningIcon } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Callout } from "@/components/ui/callout";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { ClipGroupList } from "@/features/ingest/components/clip-group-list";
import { useCardOffer } from "@/features/ingest/hooks/use-card-offer";
import {
	defaultProjectSlug,
	formatBytes,
	hasEnoughFreeSpace,
	selectionTotals,
} from "@/features/ingest/model/select";
import { useIngestSheetStore } from "@/features/ingest/store/ingest-sheet";
import { useDriveStatus } from "@/hooks/use-drive-status";
import { startIngest } from "@/lib/ipc/ingest";
import { createProject, listProjects, projectsKeys } from "@/lib/ipc/projects";

const GIB = 1024 ** 3;

export function ImportSheet() {
	const open = useIngestSheetStore((s) => s.open);
	const setOpen = useIngestSheetStore((s) => s.setOpen);
	const { data: offer } = useCardOffer();
	const { data: projects = [] } = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});
	const { data: drive } = useDriveStatus();
	const queryClient = useQueryClient();

	const today = new Date().toISOString().slice(0, 10);
	const [projectSlug, setProjectSlug] = useState<string | null>(null);
	const activeSlug = projectSlug ?? defaultProjectSlug(projects, today);

	const [selected, setSelected] = useState<ReadonlySet<string>>(
		() => new Set(),
	);
	useEffect(() => {
		if (offer) {
			setSelected(
				new Set(
					offer.groups.flatMap((g) =>
						g.clips.filter((c) => c.selected).map((c) => c.path),
					),
				),
			);
		}
	}, [offer]);

	const [creating, setCreating] = useState(false);
	const [newTitle, setNewTitle] = useState("");
	const createMutation = useMutation({
		mutationFn: () => createProject(newTitle.trim()),
		onSuccess: (project) => {
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
			setProjectSlug(project.slug);
			setCreating(false);
			setNewTitle("");
		},
		onError: (err) => toast.error(err.message),
	});

	const totals = offer
		? selectionTotals(offer, selected)
		: { count: 0, bytes: 0 };
	const activeProject = projects.find((p) => p.slug === activeSlug);
	const driveMounted = drive?.mounted ?? true;
	const freeBytes =
		drive?.free_gb !== null && drive?.free_gb !== undefined
			? drive.free_gb * GIB
			: null;
	const spaceShort =
		freeBytes !== null && !hasEnoughFreeSpace(totals.bytes, freeBytes);

	const importMutation = useMutation({
		mutationFn: () => {
			if (!offer || !activeSlug) throw new Error("no card or project");
			return startIngest(offer.volume, activeSlug, [...selected]);
		},
		onSuccess: () => {
			void queryClient.invalidateQueries({ queryKey: ["jobs"] });
			setOpen(false);
			toast.success(`Importing ${totals.count} clips`);
		},
		onError: (err) => toast.error(err.message),
	});

	if (!offer) return null;

	const toggle = (path: string, on: boolean) =>
		setSelected((prev) => {
			const next = new Set(prev);
			if (on) next.add(path);
			else next.delete(path);
			return next;
		});
	const toggleGroup = (paths: string[], on: boolean) =>
		setSelected((prev) => {
			const next = new Set(prev);
			for (const p of paths) {
				if (on) next.add(p);
				else next.delete(p);
			}
			return next;
		});

	return (
		<Dialog open={open} onOpenChange={setOpen}>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle>Import {totals.count} clips</DialogTitle>
				</DialogHeader>

				<div className="flex flex-col gap-4">
					<div className="flex items-center gap-2">
						<Select
							value={activeSlug ?? undefined}
							onValueChange={setProjectSlug}
						>
							<SelectTrigger className="flex-1">
								<SelectValue placeholder="Choose a project" />
							</SelectTrigger>
							<SelectContent>
								{projects.map((p) => (
									<SelectItem key={p.slug} value={p.slug}>
										{p.title}
									</SelectItem>
								))}
							</SelectContent>
						</Select>
						<Button
							variant="ghost"
							size="sm"
							onClick={() => setCreating((v) => !v)}
						>
							New project
						</Button>
					</div>

					{creating && (
						<div className="flex items-center gap-2">
							<Input
								value={newTitle}
								onChange={(e) => setNewTitle(e.target.value)}
								placeholder="Project title"
								aria-label="New project title"
							/>
							<Button
								variant="secondary"
								size="sm"
								disabled={newTitle.trim() === "" || createMutation.isPending}
								onClick={() => createMutation.mutate()}
							>
								Create
							</Button>
						</div>
					)}

					<div className="max-h-72 overflow-y-auto">
						<ClipGroupList
							offer={offer}
							selected={selected}
							onToggle={toggle}
							onToggleGroup={toggleGroup}
						/>
					</div>

					<div className="flex items-center justify-between text-fg-muted text-sm">
						<span>{totals.count} selected</span>
						<span className="font-mono tabular-nums">
							{formatBytes(totals.bytes)}
						</span>
					</div>

					{!driveMounted && (
						<Callout>
							<WarningIcon size={16} className="mt-0.5 text-warn" />
							<span>
								The studio drive is disconnected. Reconnect it before importing.
							</span>
						</Callout>
					)}

					{driveMounted && spaceShort && freeBytes !== null && (
						<Callout>
							<WarningIcon size={16} className="mt-0.5 text-warn" />
							<span>
								Not enough space on the studio drive:{" "}
								{formatBytes(totals.bytes)} selected, {formatBytes(freeBytes)}{" "}
								free.
							</span>
						</Callout>
					)}

					<div className="flex justify-end gap-2">
						<Button variant="secondary" onClick={() => setOpen(false)}>
							Cancel
						</Button>
						<Button
							onClick={() => importMutation.mutate()}
							disabled={
								totals.count === 0 ||
								!activeSlug ||
								!driveMounted ||
								importMutation.isPending
							}
						>
							Import to {activeProject?.title ?? "project"}
						</Button>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}
