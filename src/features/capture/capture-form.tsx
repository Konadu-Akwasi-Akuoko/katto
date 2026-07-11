import { useMutation } from "@tanstack/react-query";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { captureSubmit } from "@/lib/ipc/ideas";

const KINDS = [
	{ value: "unset", label: "Unsorted" },
	{ value: "long", label: "Long-form" },
	{ value: "short", label: "Short" },
	{ value: "series", label: "Series" },
] as const;

/**
 * The quick-capture window's only surface: catch a title (plus an optional note
 * and kind) and drop it into the backlog. Enter submits, Escape cancels — both
 * close the window. Rendered standalone for the `capture` window label.
 */
export function CaptureForm() {
	const [title, setTitle] = useState("");
	const [note, setNote] = useState("");
	const [kind, setKind] = useState("unset");
	const trimmed = title.trim();

	const submit = useMutation({
		mutationFn: () =>
			captureSubmit(
				trimmed,
				note.trim() || null,
				kind === "unset" ? null : kind,
			),
		onSuccess: () => void getCurrentWindow().close(),
	});

	function close() {
		void getCurrentWindow().close();
	}

	return (
		<form
			className="grain flex h-dvh flex-col gap-2 overflow-hidden rounded-lg border bg-surface p-3 shadow-[var(--shadow)]"
			onSubmit={(event) => {
				event.preventDefault();
				if (!trimmed || submit.isPending) return;
				submit.mutate();
			}}
			onKeyDown={(event) => {
				if (event.key === "Escape") {
					event.preventDefault();
					close();
				}
			}}
		>
			<Input
				autoFocus
				aria-label="Idea"
				placeholder="What's the idea?"
				value={title}
				onChange={(event) => setTitle(event.target.value)}
			/>
			<Input
				aria-label="Note"
				placeholder="Add a note (optional)"
				className="text-xs"
				value={note}
				onChange={(event) => setNote(event.target.value)}
			/>
			<div className="mt-auto flex items-center justify-between gap-2">
				<Select value={kind} onValueChange={setKind}>
					<SelectTrigger size="sm" aria-label="Kind" className="w-32">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						{KINDS.map((option) => (
							<SelectItem key={option.value} value={option.value}>
								{option.label}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
				<div className="flex items-center gap-2">
					<span className="text-xs text-fg-faint">Esc to cancel</span>
					<Button
						type="submit"
						size="sm"
						disabled={!trimmed || submit.isPending}
					>
						Capture
					</Button>
				</div>
			</div>
		</form>
	);
}
