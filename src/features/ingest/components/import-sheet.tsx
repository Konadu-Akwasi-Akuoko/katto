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
import { IngestProgress } from "@/features/ingest/components/ingest-progress";
import { useCardOffer } from "@/features/ingest/hooks/use-card-offer";
import {
	clipCountLabel,
	defaultProjectSlug,
	formatBytes,
	hasEnoughFreeSpace,
	selectionTotals,
} from "@/features/ingest/model/select";
import { useDriveStatus } from "@/hooks/use-drive-status";
import { startIngest } from "@/lib/ipc/ingest";
import { jobsKeys } from "@/lib/ipc/jobs";
import { createProject, listProjects, projectsKeys } from "@/lib/ipc/projects";
import { useIngestSheetStore } from "@/stores/ingest-sheet";

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

	// Set when an import has been started from this sheet: the body swaps from
	// the clip list to the copy-progress panel (which offers Eject on finish).
	// Carries everything the panel needs so it survives the card offer going
	// null when the volume is ejected or removed.
	const [activeJob, setActiveJob] = useState<{
		id: string;
		count: number;
		volume: string;
		projectTitle: string;
	} | null>(null);

	const importMutation = useMutation({
		mutationFn: () => {
			if (!offer || !activeSlug) throw new Error("no card or project");
			return startIngest(offer.volume, activeSlug, [...selected]);
		},
		onSuccess: (job) => {
			void queryClient.invalidateQueries({ queryKey: jobsKeys.all });
			if (!offer) return;
			setActiveJob({
				id: job.id,
				count: totals.count,
				volume: offer.volume,
				projectTitle: activeProject?.title ?? "project",
			});
		},
		onError: (err) => toast.error(err.message),
	});

	if (activeJob) {
		return (
			<Dialog
				open={open}
				onOpenChange={(next) => {
					setOpen(next);
					if (!next) setActiveJob(null);
				}}
			>
				<DialogContent className="max-w-lg">
					<DialogHeader>
						<DialogTitle>Importing footage</DialogTitle>
					</DialogHeader>
					<IngestProgress
						jobId={activeJob.id}
						volume={activeJob.volume}
						projectTitle={activeJob.projectTitle}
						clipCount={activeJob.count}
					/>
					<div className="flex justify-end">
						<Button
							variant="secondary"
							onClick={() => {
								setOpen(false);
								setActiveJob(null);
							}}
						>
							Close
						</Button>
					</div>
				</DialogContent>
			</Dialog>
		);
	}

	if (!offer) {
		// Opened by the palette or katto://ingest with no card inserted: say so
		// instead of silently doing nothing (and let the user clear the state).
		return (
			<Dialog open={open} onOpenChange={setOpen}>
				<DialogContent className="max-w-lg">
					<DialogHeader>
						<DialogTitle>No camera card detected</DialogTitle>
					</DialogHeader>
					<p className="text-fg-muted text-sm">
						Insert a card — the import sheet opens by itself when one is
						recognized.
					</p>
					<div className="flex justify-end">
						<Button variant="secondary" onClick={() => setOpen(false)}>
							Close
						</Button>
					</div>
				</DialogContent>
			</Dialog>
		);
	}

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
					<DialogTitle>Import {clipCountLabel(totals.count)}</DialogTitle>
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
