import { useMutation, useQueryClient } from "@tanstack/react-query";
import { type ReactNode, useState } from "react";
import { Button } from "@/components/ui/button";
import { DateInput } from "@/components/ui/date-input";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
import { useDriveStatus } from "@/hooks/use-drive-status";
import { calendarKeys } from "@/lib/ipc/calendar";
import type { Project } from "@/lib/ipc/projects";
import { projectsKeys } from "@/lib/ipc/projects";
import {
	deleteScheduleEntry,
	type ScheduleKind,
	scheduleKeys,
	upsertScheduleEntry,
} from "@/lib/ipc/schedule";
import { cn } from "@/lib/utils";

/** Add a new pin on `date`, or edit the existing pin the trigger represents. */
export type PinContext =
	| { mode: "add"; date: string }
	| {
			mode: "edit";
			date: string;
			projectSlug: string;
			kind: ScheduleKind;
			note: string | null;
	  };

const KINDS: ReadonlyArray<{ value: ScheduleKind; label: string }> = [
	{ value: "shoot", label: "Shoot" },
	{ value: "publish", label: "Publish" },
];

/** A popover for adding, moving, or clearing a shoot/publish pin from the calendar. */
export function PinPopover({
	ctx,
	projects,
	children,
}: {
	ctx: PinContext;
	projects: Project[];
	children: ReactNode;
}) {
	const [open, setOpen] = useState(false);
	return (
		<Popover open={open} onOpenChange={setOpen}>
			<PopoverTrigger asChild>{children}</PopoverTrigger>
			<PopoverContent className="w-64">
				<PinForm ctx={ctx} projects={projects} onDone={() => setOpen(false)} />
			</PopoverContent>
		</Popover>
	);
}

function PinForm({
	ctx,
	projects,
	onDone,
}: {
	ctx: PinContext;
	projects: Project[];
	onDone: () => void;
}) {
	const queryClient = useQueryClient();
	const mounted = useDriveStatus().data?.mounted ?? false;

	const [kind, setKind] = useState<ScheduleKind>(
		ctx.mode === "edit" ? ctx.kind : "shoot",
	);
	const [project, setProject] = useState<string | null>(
		ctx.mode === "edit" ? ctx.projectSlug : null,
	);
	const [date, setDate] = useState<string | null>(ctx.date);
	const [note, setNote] = useState(ctx.mode === "edit" ? (ctx.note ?? "") : "");

	const invalidate = () => {
		void queryClient.invalidateQueries({ queryKey: calendarKeys.all });
		void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
		void queryClient.invalidateQueries({ queryKey: scheduleKeys.all });
	};

	const save = useMutation({
		mutationFn: async () => {
			if (project === null || date === null) return;
			await upsertScheduleEntry(project, kind, date, note.trim() || null);
		},
		onSuccess: () => {
			invalidate();
			onDone();
		},
	});

	const clear = useMutation({
		mutationFn: async () => {
			if (ctx.mode !== "edit") return;
			await deleteScheduleEntry(ctx.projectSlug, ctx.kind);
		},
		onSuccess: () => {
			invalidate();
			onDone();
		},
	});

	const canSave =
		mounted && project !== null && date !== null && !save.isPending;

	return (
		<div className="flex flex-col gap-3">
			<div className="flex flex-col gap-1.5">
				<Label>Kind</Label>
				<div className="inline-flex overflow-hidden rounded-md border">
					{KINDS.map((k) => (
						<button
							key={k.value}
							type="button"
							disabled={ctx.mode === "edit"}
							onClick={() => setKind(k.value)}
							className={cn(
								"h-8 flex-1 cursor-default border-r px-3 text-xs font-medium last:border-r-0 disabled:opacity-45",
								k.value === kind
									? "bg-ember/15 text-ember"
									: "bg-transparent text-fg-muted hover:text-fg",
							)}
						>
							{k.label}
						</button>
					))}
				</div>
			</div>

			<div className="flex flex-col gap-1.5">
				<Label htmlFor="pin-project">Project</Label>
				<Select
					value={project ?? undefined}
					disabled={ctx.mode === "edit"}
					onValueChange={setProject}
				>
					<SelectTrigger id="pin-project" size="sm">
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
			</div>

			<div className="flex flex-col gap-1.5">
				<Label>Date</Label>
				<DateInput value={date} onValueChange={setDate} label="pin" />
			</div>

			<div className="flex flex-col gap-1.5">
				<Label htmlFor="pin-note">Note</Label>
				<Input
					id="pin-note"
					placeholder="Optional"
					value={note}
					onChange={(e) => setNote(e.target.value)}
				/>
			</div>

			{!mounted && (
				<p className="text-xs text-fg-faint">
					The studio drive is unmounted — pins are read-only.
				</p>
			)}

			<div className="flex items-center justify-between">
				{ctx.mode === "edit" ? (
					<Button
						variant="ghost"
						size="sm"
						disabled={!mounted || clear.isPending}
						onClick={() => clear.mutate()}
					>
						Clear
					</Button>
				) : (
					<span />
				)}
				<Button size="sm" disabled={!canSave} onClick={() => save.mutate()}>
					{ctx.mode === "edit" ? "Save" : "Add"}
				</Button>
			</div>
		</div>
	);
}
