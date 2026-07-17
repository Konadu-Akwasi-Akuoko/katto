import { Badge } from "@/components/ui/badge";
import { statusAppearance } from "@/lib/appearance";
import { cn } from "@/lib/utils";

/**
 * A project's pipeline stage as one dot-chip: status tint behind the status
 * colour, with the curated label — an unknown status reads as "Idea", matching
 * how the board buckets it, rather than echoing a stale string back. The dot
 * takes its colour from the chip's own `currentColor` (the Badge cva hard-wires
 * `[&>.dot]:bg-current` at a specificity a plain `bg-*` utility can't beat), so
 * the colour is set once on the chip and encoded once on the card.
 */
export function StatusChip({ status }: { status: string }) {
	const { label, fg, tint } = statusAppearance(status);
	return (
		<Badge variant="ghost" className={cn("shrink-0", tint, fg)}>
			<span className="dot" />
			{label}
		</Badge>
	);
}
