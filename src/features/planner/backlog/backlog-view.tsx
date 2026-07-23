import {
	ArrowUpRightIcon,
	LightbulbIcon,
	TrashIcon,
} from "@phosphor-icons/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatShortDate } from "@/lib/date";
import type { Idea } from "@/lib/ipc/ideas";
import {
	createIdea,
	discardIdea,
	ideasKeys,
	listIdeas,
	promoteIdea,
} from "@/lib/ipc/ideas";
import { projectsKeys } from "@/lib/ipc/projects";
import { KIND_LABELS } from "@/lib/kind";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/ui";
import { IdeaDetailModal } from "./idea-detail-modal";
import { type Lean, parseLean, sourceDomain } from "./model/lean";

export function BacklogView() {
	const queryClient = useQueryClient();
	const { data: ideas } = useQuery({
		queryKey: ideasKeys.byStatus("backlog"),
		queryFn: () => listIdeas("backlog"),
	});
	const [openIdea, setOpenIdea] = useState<Idea | null>(null);

	const invalidateIdeas = () =>
		queryClient.invalidateQueries({ queryKey: ideasKeys.all });

	const create = useMutation({
		mutationFn: (title: string) =>
			createIdea({ title, kind: null, notes: null }),
		onSuccess: () => void invalidateIdeas(),
	});
	const discard = useMutation({
		mutationFn: (id: string) => discardIdea(id),
		onSuccess: () => void invalidateIdeas(),
	});
	const promote = useMutation({
		mutationFn: (id: string) => promoteIdea(id),
		onSuccess: (result) => {
			useUiStore.getState().setJustPromoted(result.slug);
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
				<EmptyState />
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
							onOpen={() => setOpenIdea(idea)}
							onPromote={() => promote.mutate(idea.id)}
							onDiscard={() => discard.mutate(idea.id)}
						/>
					))}
				</ul>
			)}

			{openIdea !== null && (
				<IdeaDetailModal idea={openIdea} onClose={() => setOpenIdea(null)} />
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
	onOpen,
	onPromote,
	onDiscard,
}: {
	idea: Idea;
	leaving: "promote" | "discard" | null;
	onOpen: () => void;
	onPromote: () => void;
	onDiscard: () => void;
}) {
	const lean = parseLean(idea.evidence_json);
	const domain = sourceDomain(idea.source_url);
	const secondary = idea.rationale ?? idea.notes;
	const suggested = idea.kind_source === "ai";

	return (
		<li
			className={cn(
				"grain group flex items-center gap-3 rounded-lg border bg-surface px-3 py-2 transition-[opacity,transform] duration-(--dur) ease-(--ease) motion-reduce:transition-none",
				leaving === "promote" && "-translate-y-2 opacity-0",
				leaving === "discard" && "translate-x-2 opacity-0",
			)}
		>
			<div className="flex w-3 shrink-0 justify-center">
				{lean !== null && <LeanNotch lean={lean} />}
			</div>

			<button
				type="button"
				onClick={onOpen}
				className="flex min-w-0 flex-1 flex-col gap-0.5 rounded-sm text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember"
			>
				<span className="truncate text-sm text-fg">
					{idea.title || <span className="text-fg-faint">Untitled idea</span>}
				</span>
				{secondary !== null && secondary !== "" && (
					<span className="truncate text-xs text-fg-muted">{secondary}</span>
				)}
				<span className="flex min-h-4 items-center gap-2 text-xs text-fg-faint">
					{domain !== null && <span className="truncate">{domain}</span>}
					<span className="font-mono tabular-nums">
						{formatShortDate(idea.first_seen)}
					</span>
				</span>
			</button>

			<span
				className={cn(
					"inline-flex h-[22px] shrink-0 items-center gap-1.5 rounded-md border bg-surface-2 px-2 text-xs font-medium",
					idea.kind === "unset" ? "text-fg-faint" : "text-fg-muted",
				)}
				title={suggested ? (idea.kind_why ?? undefined) : undefined}
			>
				<span
					className={cn(
						"size-1.5 rounded-full",
						suggested ? "bg-ember" : "bg-current opacity-80",
					)}
				/>
				{KIND_LABELS[idea.kind] ?? idea.kind}
			</span>

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
				className="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
				disabled={leaving !== null}
				onClick={onDiscard}
			>
				<TrashIcon />
			</Button>
		</li>
	);
}

function EmptyState() {
	return (
		<div className="flex flex-col items-center gap-1 rounded-lg border border-dashed py-11 text-center">
			<div className="mb-3 flex size-14 items-center justify-center rounded-[10px] border bg-surface-2 text-fg-faint">
				<LightbulbIcon size={26} />
			</div>
			<p className="text-sm text-fg-muted">Nothing banked yet</p>
			<p className="max-w-sm text-xs text-fg-faint">
				Capture an idea above — the hotkey works in any app. The curation run
				drops its keepers here too.
			</p>
		</div>
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
	);
}
