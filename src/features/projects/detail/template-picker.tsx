import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import type { ThumbFormat } from "@/lib/ipc/thumbnails";

/**
 * Two-option template picker: proportional outline rectangles drawn with
 * borders — no images, the dimensions are the content.
 */
export function TemplatePicker({
	open,
	onOpenChange,
	onPick,
}: {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onPick: (format: ThumbFormat) => void;
}) {
	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="max-w-sm">
				<DialogHeader>
					<DialogTitle>New thumbnail</DialogTitle>
					<DialogDescription>
						Scaffolds a guide-lined PSD in the project's thumbnails folder.
					</DialogDescription>
				</DialogHeader>
				<div className="flex items-end justify-center gap-6 py-2">
					<button
						type="button"
						onClick={() => onPick("landscape")}
						className="flex flex-col items-center gap-2 rounded-md p-3 hover:bg-surface-2"
					>
						<span className="h-[45px] w-20 rounded-sm border-2 border-fg-muted" />
						<span className="text-xs text-fg">YouTube</span>
						<span className="text-[11px] text-fg-faint tabular-nums">
							1280 × 720
						</span>
					</button>
					<button
						type="button"
						onClick={() => onPick("portrait")}
						className="flex flex-col items-center gap-2 rounded-md p-3 hover:bg-surface-2"
					>
						<span className="h-20 w-[45px] rounded-sm border-2 border-fg-muted" />
						<span className="text-xs text-fg">Vertical</span>
						<span className="text-[11px] text-fg-faint tabular-nums">
							1080 × 1920
						</span>
					</button>
				</div>
			</DialogContent>
		</Dialog>
	);
}
