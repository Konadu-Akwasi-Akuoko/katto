import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";
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
import { createVfxEffect, vfxKeys } from "@/lib/ipc/vfx";
import { useUiStore } from "@/stores/ui";

/**
 * Name an effect; creating it scaffolds `assets/vfx/<slug>/`, spawns a Claude
 * session in the folder, and focuses the dock on it.
 */
export function NewEffectDialog({
	slug,
	open,
	onOpenChange,
}: {
	slug: string;
	open: boolean;
	onOpenChange: (open: boolean) => void;
}) {
	const inputId = useId();
	const [name, setName] = useState("");
	const queryClient = useQueryClient();
	const openDock = useUiStore((s) => s.openDock);

	const create = useMutation({
		mutationFn: (effectName: string) => createVfxEffect(slug, effectName),
		onSuccess: (sessionId) => {
			onOpenChange(false);
			setName("");
			void queryClient.invalidateQueries({ queryKey: vfxKeys.byProject(slug) });
			openDock(sessionId);
		},
	});

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="max-w-sm">
				<DialogHeader>
					<DialogTitle>New effect</DialogTitle>
				</DialogHeader>
				<div className="flex flex-col gap-2">
					<Label htmlFor={inputId}>Effect name</Label>
					<Input
						id={inputId}
						value={name}
						onChange={(e) => setName(e.target.value)}
						placeholder="Intro glitch"
						autoFocus
					/>
				</div>
				<DialogFooter>
					<Button
						onClick={() => create.mutate(name)}
						disabled={name.trim() === "" || create.isPending}
					>
						Create
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
