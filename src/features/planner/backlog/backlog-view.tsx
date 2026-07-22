import { ArrowUpRightIcon, CheckIcon, TrashIcon } from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import type { Idea, IdeaPatch } from "@/lib/ipc/ideas";
import {
	createIdea,
	discardIdea,
	ideasKeys,
	listIdeas,
	promoteIdea,
	updateIdea,
} from "@/lib/ipc/ideas";
import { projectsKeys } from "@/lib/ipc/projects";
import { openExternalUrl } from "@/lib/ipc/shell";
import { cn } from "@/lib/utils";
import { type Lean, parseLean, sourceDomain } from "./model/lean";

const KINDS = [
	{ value: "unset", label: "Unsorted" },
	{ value: "long", label: "Long-form" },
	{ value: "short", label: "Short" },
	{ value: "series", label: "Series" },
] as const;

export function BacklogView() {
	const queryClient = useQueryClient();
	const { data: ideas } = useQuery({
		queryKey: ideasKeys.byStatus("backlog"),
		queryFn: () => listIdeas("backlog"),
	});

	const invalidateIdeas = () =>
		queryClient.invalidateQueries({ queryKey: ideasKeys.all });

	const create = useMutation({
		mutationFn: (title: string) =>
			createIdea({ title, kind: null, notes: null }),
		onSuccess: () => void invalidateIdeas(),
	});
	const patch = useMutation({
		mutationFn: ({ id, patch }: { id: string; patch: IdeaPatch }) =>
			updateIdea(id, patch),
		onSuccess: () => void invalidateIdeas(),
	});
	const discard = useMutation({
		mutationFn: (id: string) => discardIdea(id),
		onSuccess: () => void invalidateIdeas(),
	});
	const promote = useMutation({
		mutationFn: (id: string) => promoteIdea(id),
		onSuccess: () => {
			void invalidateIdeas();
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
		},
	});

	const promotingId = promote.isPending ? promote.variables : null;
	const discardingId = discard.isPending ? discard.variables : null;

	return (
		<div className="flex h-full flex-col gap-4">
			<CaptureRow
				pending={create.isPending}
				onCapture={(title) => create.mutate(title)}
			/>

			{ideas === undefined ? null : ideas.length === 0 ? (
				<p className="text-sm text-fg-muted">
					Nothing in the backlog. Capture an idea above, and it lands here to
					triage.
				</p>
			) : (
				<ul className="flex flex-col gap-2">
					{ideas.map((idea) => (
						<IdeaRow
							key={idea.id}
							idea={idea}
							leaving={
								idea.id === promotingId
									? "promote"
									: idea.id === discardingId
										? "discard"
										: null
							}
							onPatch={(next) => patch.mutate({ id: idea.id, patch: next })}
							onPromote={() => promote.mutate(idea.id)}
							onDiscard={() => discard.mutate(idea.id)}
						/>
					))}
				</ul>
			)}
		</div>
	);
}

function CaptureRow({
	pending,
	onCapture,
}: {
	pending: boolean;
	onCapture: (title: string) => void;
}) {
	const [draft, setDraft] = useState("");
	const trimmed = draft.trim();

	return (
		<form
			className="flex gap-2"
			onSubmit={(event) => {
				event.preventDefault();
				if (!trimmed) return;
				onCapture(trimmed);
				setDraft("");
			}}
		>
			<Input
				aria-label="New idea"
				placeholder="Capture an idea"
				value={draft}
				onChange={(event) => setDraft(event.target.value)}
			/>
			<Button type="submit" disabled={!trimmed || pending}>
				Add
			</Button>
		</form>
	);
}

function IdeaRow({
	idea,
	leaving,
	onPatch,
	onPromote,
	onDiscard,
}: {
	idea: Idea;
	leaving: "promote" | "discard" | null;
	onPatch: (patch: IdeaPatch) => void;
	onPromote: () => void;
	onDiscard: () => void;
}) {
	const lean = parseLean(idea.evidence_json);
	const domain = sourceDomain(idea.source_url);
	const openSource = useMutation({ mutationFn: openExternalUrl });

	return (
		<li
			className={cn(
				"grain flex items-center gap-3 rounded-lg border bg-surface px-3 py-2 transition-[opacity,transform] duration-200 ease-out motion-reduce:transition-none",
				leaving === "promote" && "-translate-y-2 opacity-0",
				leaving === "discard" && "translate-x-2 opacity-0",
			)}
		>
			<div className="flex min-w-0 flex-1 flex-col gap-0.5">
				<EditableText
					value={idea.title}
					label="Idea title"
					placeholder="Untitled idea"
					className="text-sm text-fg"
					onCommit={(title) =>
						onPatch({ title, kind: null, notes: null, kind_source: null })
					}
				/>
				{idea.rationale !== null && (
					<p className="truncate text-xs text-fg-muted" title={idea.rationale}>
						{idea.rationale}
					</p>
				)}
				<EditableText
					value={idea.notes ?? ""}
					label="Idea note"
					placeholder="Add a note"
					className="text-xs text-fg-muted"
					onCommit={(notes) =>
						onPatch({ title: null, kind: null, notes, kind_source: null })
					}
				/>
				{domain !== null && idea.source_url !== null && (
					<button
						type="button"
						className="self-start truncate text-xs text-fg-faint hover:text-fg-muted"
						title={idea.source_url}
						onClick={() => {
							if (idea.source_url !== null) openSource.mutate(idea.source_url);
						}}
					>
						{domain}
					</button>
				)}
			</div>

			{lean !== null && <LeanNotch lean={lean} />}

			{idea.kind_source === "ai" && (
				<span
					className="shrink-0 text-xs text-fg-faint"
					title={idea.kind_why ?? undefined}
				>
					suggested
				</span>
			)}

			<Select
				value={idea.kind}
				onValueChange={(kind) =>
					onPatch({ title: null, kind, notes: null, kind_source: "human" })
				}
			>
				<SelectTrigger size="sm" aria-label="Kind" className="w-32 shrink-0">
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

			{idea.kind_source === "ai" && (
				<Button
					variant="ghost"
					size="icon-sm"
					aria-label="Keep suggested kind"
					title={idea.kind_why ?? "Keep the suggested kind"}
					onClick={() =>
						onPatch({
							title: null,
							kind: idea.kind,
							notes: null,
							kind_source: "human",
						})
					}
				>
					<CheckIcon />
				</Button>
			)}

			<Button
				variant="secondary"
				size="sm"
				disabled={leaving !== null}
				onClick={onPromote}
			>
				<ArrowUpRightIcon />
				Promote
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				aria-label={`Discard ${idea.title}`}
				disabled={leaving !== null}
				onClick={onDiscard}
			>
				<TrashIcon />
			</Button>
		</li>
	);
}

const LEAN_STEPS: Record<Lean, number> = { hold: 1, lean: 2, strong: 3 };

/**
 * The curation run's lean as a three-bar notch — deliberately not a number
 * (no scoring anywhere: AI suggests, the human decides).
 */
function LeanNotch({ lean }: { lean: Lean }) {
	const filled = LEAN_STEPS[lean];
	return (
		<div
			role="img"
			aria-label={`lean: ${lean}`}
			title={`lean: ${lean}`}
			className="flex shrink-0 flex-col-reverse gap-0.5"
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
	);
}

function EditableText({
	value,
	label,
	placeholder,
	className,
	onCommit,
}: {
	value: string;
	label: string;
	placeholder: string;
	className?: string;
	onCommit: (value: string) => void;
}) {
	const [editing, setEditing] = useState(false);
	const [draft, setDraft] = useState(value);
	const inputRef = useRef<HTMLInputElement>(null);
	const fieldId = useId();

	useEffect(() => {
		if (editing) inputRef.current?.focus();
	}, [editing]);

	function commit() {
		setEditing(false);
		const next = draft.trim();
		if (next !== value) onCommit(next);
	}

	if (editing) {
		return (
			<input
				ref={inputRef}
				id={fieldId}
				aria-label={label}
				value={draft}
				onChange={(event) => setDraft(event.target.value)}
				onBlur={commit}
				onKeyDown={(event) => {
					if (event.key === "Enter") {
						event.preventDefault();
						commit();
					}
					if (event.key === "Escape") {
						setDraft(value);
						setEditing(false);
					}
				}}
				className={cn(
					"w-full rounded-sm bg-transparent outline-none focus-visible:ring-2 focus-visible:ring-ember",
					className,
				)}
			/>
		);
	}

	return (
		<button
			type="button"
			className={cn("truncate text-left", className)}
			onClick={() => {
				setDraft(value);
				setEditing(true);
			}}
		>
			{value || <span className="text-fg-faint">{placeholder}</span>}
		</button>
	);
}
