import { FilmStripIcon } from "@phosphor-icons/react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { importFiles } from "@/lib/ipc/ingest";
import { jobsKeys } from "@/lib/ipc/jobs";

const VIDEO_EXTS = new Set(["mp4", "mov", "mts", "m4v"]);

/** Keep only paths with a video extension (folders and sidecars drop out). */
function videoPaths(paths: string[]): string[] {
	return paths.filter((p) =>
		VIDEO_EXTS.has(p.split(".").pop()?.toLowerCase() ?? ""),
	);
}

/**
 * The manual iPhone-footage path: drop video files anywhere over the detail
 * view and they run the same rename+verify ingest pipeline as a card import —
 * no watcher involvement. Uses the Tauri webview drag-drop event because a DOM
 * drop would not carry absolute paths.
 */
export function FootageCard({ slug }: { slug: string }) {
	const [over, setOver] = useState(false);
	const queryClient = useQueryClient();

	const drop = useMutation({
		mutationFn: (paths: string[]) => importFiles(slug, paths),
		onSuccess: (_job, paths) => {
			void queryClient.invalidateQueries({ queryKey: jobsKeys.all });
			toast.success(
				`Importing ${paths.length} ${paths.length === 1 ? "file" : "files"}`,
			);
		},
		onError: (err) => toast.error(err.message),
	});

	useEffect(() => {
		let cancelled = false;
		let webview: ReturnType<typeof getCurrentWebview>;
		try {
			webview = getCurrentWebview();
		} catch {
			// Outside a real Tauri webview (jsdom tests) there is no drop source.
			return;
		}
		const unlisten = webview.onDragDropEvent((event) => {
			if (cancelled) return;
			if (event.payload.type === "enter") setOver(true);
			else if (event.payload.type === "leave") setOver(false);
			else if (event.payload.type === "drop") {
				setOver(false);
				if (drop.isPending) return;
				const videos = videoPaths(event.payload.paths);
				if (videos.length === 0) {
					toast.error("No video files in that drop");
					return;
				}
				drop.mutate(videos);
			}
		});
		return () => {
			cancelled = true;
			void unlisten.then((un) => un());
		};
	}, [drop.isPending, drop.mutate]);

	return (
		<Card className={over ? "border-ember" : undefined}>
			<CardHeader>
				<CardTitle>Footage</CardTitle>
			</CardHeader>
			<CardContent>
				<div className="flex items-center gap-2 text-fg-muted">
					<FilmStripIcon size={20} />
					<span className="text-sm">
						Drop iPhone footage here to import it into this project
					</span>
				</div>
			</CardContent>
		</Card>
	);
}
