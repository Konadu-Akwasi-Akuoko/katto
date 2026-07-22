import { FilmStripIcon } from "@phosphor-icons/react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { importFiles } from "@/lib/ipc/ingest";

/**
 * The manual iPhone-footage path: drop video files anywhere over the detail
 * view and they run the same rename+verify ingest pipeline as a card import —
 * no watcher involvement. Uses the Tauri webview drag-drop event because a DOM
 * drop would not carry absolute paths.
 */
export function FootageCard({ slug }: { slug: string }) {
	const [over, setOver] = useState(false);

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
				const paths = event.payload.paths;
				void importFiles(slug, paths)
					.then(() => toast.success(`Importing ${paths.length} files`))
					.catch((err: Error) => toast.error(err.message));
			}
		});
		return () => {
			cancelled = true;
			void unlisten.then((un) => un());
		};
	}, [slug]);

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
