import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { formatShortDate } from "@/lib/date";
import type { Idea } from "@/lib/ipc/ideas";
import { ideasKeys, updateIdea } from "@/lib/ipc/ideas";
import { cn } from "@/lib/utils";
import { type Lean, parseLean, sourceDomain } from "./model/lean";

const KINDS = [
	{ value: "unset", label: "Unsorted" },
	{ value: "long", label: "Long-form" },
	{ value: "short", label: "Short" },
	{ value: "series", label: "Series" },
] as const;

type LeanChoice = Lean | "none";
const SIGNAL_OPTIONS: ReadonlyArray<{ value: LeanChoice; label: string }> = [
	{ value: "none", label: "None" },
	{ value: "hold", label: "Hold" },
	{ value: "lean", label: "Lean" },
	{ value: "strong", label: "Strong" },
];
const SIGNAL_STEPS: Record<LeanChoice, number> = {
	none: 0,
	hold: 1,
	lean: 2,
	strong: 3,
};

function SignalField({
	value,
	onChange,
}: {
	value: LeanChoice;
	onChange: (value: LeanChoice) => void;
}) {
	const filled = SIGNAL_STEPS[value];
	return (
		<div className="flex items-center gap-3">
			<div className="inline-flex overflow-hidden rounded-md border">
				{SIGNAL_OPTIONS.map((option) => (
					<button
						key={option.value}
						type="button"
						onClick={() => onChange(option.value)}
						className={cn(
							"h-9 cursor-default border-r px-3 text-xs font-medium last:border-r-0",
							option.value === value
								? "bg-ember/15 text-ember"
								: "bg-transparent text-fg-muted hover:text-fg",
						)}
					>
						{option.label}
					</button>
				))}
			</div>
			<div
				role="img"
				aria-label={`signal: ${value}`}
				className="flex flex-col-reverse gap-0.5"
			>
				{[0, 1, 2].map((step) => (
					<span
						key={step}
						className={cn(
							"h-1 w-3 rounded-full",
							step < filled ? "bg-ember" : "bg-border",
						)}
					/>
				))}
			</div>
		</div>
	);
}

export function IdeaDetailModal({
	idea,
	onClose,
}: {
	idea: Idea;
	onClose: () => void;
}) {
	const queryClient = useQueryClient();
	const initial = {
		title: idea.title,
		notes: idea.notes ?? "",
		kind: idea.kind,
		rationale: idea.rationale ?? "",
		sourceUrl: idea.source_url ?? "",
		lean: (parseLean(idea.evidence_json) ?? "none") as LeanChoice,
	};
	const [form, setForm] = useState(initial);
	const set = (patch: Partial<typeof initial>) =>
		setForm((current) => ({ ...current, ...patch }));

	const dirty = (Object.keys(initial) as Array<keyof typeof initial>).some(
		(key) => form[key] !== initial[key],
	);
	const domain = sourceDomain(form.sourceUrl || null);
	const provenance = idea.kind_source === "ai" || idea.type !== "manual";

	const save = useMutation({
		mutationFn: () =>
			updateIdea(idea.id, {
				title: form.title.trim(),
				kind: form.kind,
				notes: form.notes.trim() || null,
				rationale: form.rationale.trim() || null,
				source_url: form.sourceUrl.trim() || null,
				lean: form.lean === "none" ? null : form.lean,
				kind_source: form.kind !== idea.kind ? "human" : idea.kind_source,
			}),
		onSuccess: () => {
			void queryClient.invalidateQueries({ queryKey: ideasKeys.all });
			onClose();
		},
	});

	return (
		<Dialog open onOpenChange={(open) => !open && onClose()}>
			<DialogContent className="max-w-xl">
				<DialogHeader>
					<DialogTitle className="pr-6 font-serif text-lg">
						{idea.title || "Untitled idea"}
					</DialogTitle>
					<div className="flex flex-wrap items-center gap-2 text-xs text-fg-faint">
						<span className="inline-flex h-[19px] items-center gap-1.5 rounded-md bg-surface-2 px-2 text-fg-muted">
							<span
								className={cn(
									"size-1.5 rounded-full",
									provenance ? "bg-ember" : "bg-fg-faint",
								)}
							/>
							{provenance ? "AI-curated" : "Manual"}
						</span>
						{idea.source !== null && <span>from {idea.source}</span>}
						<span className="font-mono tabular-nums">
							captured {formatShortDate(idea.first_seen)}
						</span>
					</div>
				</DialogHeader>

				<div className="flex flex-col gap-4">
					<div className="flex flex-col gap-1.5">
						<Label htmlFor="idea-title">Name</Label>
						<Input
							id="idea-title"
							value={form.title}
							onChange={(event) => set({ title: event.target.value })}
						/>
					</div>

					<div className="flex flex-col gap-1.5">
						<Label htmlFor="idea-desc">Description</Label>
						<Textarea
							id="idea-desc"
							placeholder="Add a note or outline…"
							value={form.notes}
							onChange={(event) => set({ notes: event.target.value })}
						/>
					</div>

					<div className="flex flex-col gap-1.5">
						<Label htmlFor="idea-kind">Kind</Label>
						<Select value={form.kind} onValueChange={(kind) => set({ kind })}>
							<SelectTrigger id="idea-kind" size="sm" className="w-40">
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								{KINDS.map((kind) => (
									<SelectItem key={kind.value} value={kind.value}>
										{kind.label}
									</SelectItem>
								))}
							</SelectContent>
						</Select>
					</div>

					<div className="flex flex-col gap-1.5">
						<Label>Signal</Label>
						<SignalField value={form.lean} onChange={(lean) => set({ lean })} />
					</div>

					<div className="flex flex-col gap-1.5">
						<Label htmlFor="idea-src">Source</Label>
						<Input
							id="idea-src"
							placeholder="https://…"
							value={form.sourceUrl}
							onChange={(event) => set({ sourceUrl: event.target.value })}
						/>
						<span className="text-xs text-fg-faint">
							{domain !== null
								? `Shows as ${domain} on the row.`
								: "Clear it to make this a plain idea."}
						</span>
					</div>

					<div className="flex flex-col gap-1.5">
						<Label htmlFor="idea-rat">Rationale</Label>
						<Textarea
							id="idea-rat"
							placeholder="Why pursue this?"
							value={form.rationale}
							onChange={(event) => set({ rationale: event.target.value })}
						/>
					</div>
				</div>

				<DialogFooter className="items-center sm:justify-between">
					<span
						className={cn(
							"flex items-center gap-1.5 text-xs text-fg-faint transition-opacity",
							dirty ? "opacity-100" : "opacity-0",
						)}
					>
						<span className="size-1.5 rounded-full bg-ember" />
						Unsaved changes
					</span>
					<div className="flex gap-2">
						<Button variant="secondary" onClick={onClose}>
							Cancel
						</Button>
						<Button
							disabled={!dirty || form.title.trim() === "" || save.isPending}
							onClick={() => save.mutate()}
						>
							Save
						</Button>
					</div>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
