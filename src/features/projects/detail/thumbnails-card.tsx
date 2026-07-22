import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { revealInProject } from "@/lib/ipc/browser";
import type { ThumbFormat } from "@/lib/ipc/thumbnails";
import {
	createThumbnail,
	latestThumbnail,
	thumbKeys,
	unwatchThumbnails,
	watchThumbnails,
} from "@/lib/ipc/thumbnails";
import { TemplatePicker } from "./template-picker";

/**
 * The project detail's Thumbnails card: scaffold a template PSD, then show
 * the newest PNG the owner exports into thumbnails/ (folder watch keeps it
 * live). Media renders via the asset protocol, never over invoke.
 */
export function ThumbnailsCard({ slug }: { slug: string }) {
	const [pickerOpen, setPickerOpen] = useState(false);
	const queryClient = useQueryClient();
	const latest = useQuery({
		queryKey: thumbKeys.latest(slug),
		queryFn: () => latestThumbnail(slug),
	});

	// this card is the single thumbnails watch (one open detail at a time)
	useEffect(() => {
		void watchThumbnails(slug);
		return () => {
			void unwatchThumbnails();
		};
	}, [slug]);

	const create = useMutation({
		mutationFn: (format: ThumbFormat) => createThumbnail(slug, format),
		onSuccess: (result) => {
			setPickerOpen(false);
			if (result.opened === "revealed_in_finder") {
				toast.info("Photoshop not found — revealed in Finder", {
					description: result.psd_path,
				});
			}
			void queryClient.invalidateQueries({ queryKey: thumbKeys.latest(slug) });
		},
	});

	const path = latest.data ?? null;
	const filename = path?.split("/").pop() ?? null;

	return (
		<Card>
			<CardHeader className="flex flex-row items-center justify-between">
				<CardTitle>Thumbnails</CardTitle>
				<Button
					variant="secondary"
					size="sm"
					onClick={() => setPickerOpen(true)}
				>
					New thumbnail
				</Button>
			</CardHeader>
			<CardContent>
				{path === null ? (
					<p className="text-sm text-fg-muted">
						No thumbnail yet. Scaffold a guide-lined PSD and katto will show the
						PNG you export.
					</p>
				) : (
					<div className="flex flex-col gap-1.5">
						<button
							type="button"
							aria-label="Reveal thumbnail in Finder"
							onClick={() => {
								if (filename !== null) {
									void revealInProject(slug, `thumbnails/${filename}`);
								}
							}}
							className="overflow-hidden rounded-md border focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2"
						>
							<img
								src={convertFileSrc(path)}
								alt={`Newest thumbnail for ${slug}`}
								className="aspect-video w-full object-cover"
							/>
						</button>
						{filename !== null && (
							<span className="truncate font-mono text-xs text-fg-muted">
								{filename}
							</span>
						)}
					</div>
				)}
			</CardContent>
			<TemplatePicker
				open={pickerOpen}
				onOpenChange={setPickerOpen}
				onPick={(format) => create.mutate(format)}
			/>
		</Card>
	);
}
