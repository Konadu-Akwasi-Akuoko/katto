import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
	CommandDialog,
	CommandEmpty,
	CommandInput,
	CommandItem,
	CommandList,
} from "@/components/ui/command";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ideasKeys, listIdeas, promoteIdea } from "@/lib/ipc/ideas";
import { createProject, listProjects, projectsKeys } from "@/lib/ipc/projects";
import { useUiStore } from "@/stores/ui";

/**
 * The secondary surfaces for the two-step palette commands. Each opens when
 * `paletteDialog` is set (see `openPaletteDialog`) and closes itself on select,
 * submit, or dismiss. Mutation failures surface through the app-wide mutation
 * error toast, so these bodies stay declarative.
 */
export function PaletteDialogs() {
	const dialog = useUiStore((s) => s.paletteDialog);
	const close = useUiStore((s) => s.closePaletteDialog);

	if (dialog === "promote-idea") return <PromoteIdeaPicker onClose={close} />;
	if (dialog === "go-to-project") return <GoToProjectPicker onClose={close} />;
	if (dialog === "new-project") return <NewProjectDialog onClose={close} />;
	return null;
}

function PromoteIdeaPicker({ onClose }: { onClose: () => void }) {
	const queryClient = useQueryClient();
	const { data: ideas } = useQuery({
		queryKey: ideasKeys.byStatus("backlog"),
		queryFn: () => listIdeas("backlog"),
	});
	const promote = useMutation({
		mutationFn: (id: string) => promoteIdea(id),
		onSuccess: (result) => {
			useUiStore.getState().setJustPromoted(result.slug);
			void queryClient.invalidateQueries({ queryKey: ideasKeys.all });
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
			onClose();
		},
	});

	return (
		<Picker
			title="Promote idea"
			description="Turn a backlog idea into a project folder."
			placeholder="Promote which idea?"
			empty="Nothing in the backlog to promote."
			onClose={onClose}
			items={(ideas ?? []).map((idea) => ({
				key: idea.id,
				value: `${idea.title} ${idea.notes ?? ""}`,
				label: idea.title,
				onSelect: () => promote.mutate(idea.id),
			}))}
		/>
	);
}

function GoToProjectPicker({ onClose }: { onClose: () => void }) {
	const openProject = useUiStore((s) => s.openProject);
	const { data: projects } = useQuery({
		queryKey: projectsKeys.all,
		queryFn: listProjects,
	});

	return (
		<Picker
			title="Go to project"
			description="Jump straight to a project's detail."
			placeholder="Go to which project?"
			empty="No projects yet."
			onClose={onClose}
			items={(projects ?? []).map((project) => ({
				key: project.slug,
				value: project.title,
				label: project.title,
				onSelect: () => {
					openProject(project.slug);
					onClose();
				},
			}))}
		/>
	);
}

type PickerItem = {
	key: string;
	value: string;
	label: string;
	onSelect: () => void;
};

function Picker({
	title,
	description,
	placeholder,
	empty,
	items,
	onClose,
}: {
	title: string;
	description: string;
	placeholder: string;
	empty: string;
	items: PickerItem[];
	onClose: () => void;
}) {
	return (
		<CommandDialog
			open
			onOpenChange={(next) => {
				if (!next) onClose();
			}}
			title={title}
			description={description}
			showCloseButton={false}
		>
			<CommandInput placeholder={placeholder} />
			<CommandList>
				<CommandEmpty>{empty}</CommandEmpty>
				{items.map((item) => (
					<CommandItem
						key={item.key}
						value={item.value}
						onSelect={item.onSelect}
					>
						{item.label}
					</CommandItem>
				))}
			</CommandList>
		</CommandDialog>
	);
}

function NewProjectDialog({ onClose }: { onClose: () => void }) {
	const queryClient = useQueryClient();
	const openProject = useUiStore((s) => s.openProject);
	const [title, setTitle] = useState("");
	const trimmed = title.trim();

	const create = useMutation({
		mutationFn: (name: string) => createProject(name),
		onSuccess: (project) => {
			void queryClient.invalidateQueries({ queryKey: projectsKeys.all });
			openProject(project.slug);
			onClose();
		},
	});

	return (
		<Dialog
			open
			onOpenChange={(next) => {
				if (!next) onClose();
			}}
		>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>New project</DialogTitle>
					<DialogDescription>
						Creates a folder in the studio root and opens it.
					</DialogDescription>
				</DialogHeader>
				<form
					className="flex flex-col gap-4"
					onSubmit={(event) => {
						event.preventDefault();
						if (!trimmed || create.isPending) return;
						create.mutate(trimmed);
					}}
				>
					<Input
						autoFocus
						aria-label="Project title"
						placeholder="Project title"
						value={title}
						onChange={(event) => setTitle(event.target.value)}
					/>
					<DialogFooter>
						<Button type="submit" disabled={!trimmed || create.isPending}>
							Create
						</Button>
					</DialogFooter>
				</form>
			</DialogContent>
		</Dialog>
	);
}
