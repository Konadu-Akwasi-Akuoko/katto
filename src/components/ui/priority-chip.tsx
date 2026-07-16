import { FlagIcon } from "@phosphor-icons/react";
import { Badge } from "@/components/ui/badge";
import { priorityAppearance } from "@/lib/appearance";
import { cn } from "@/lib/utils";

/**
 * A project's priority as one chip, in the same grammar as {@link StatusChip}.
 * Renders nothing at all for `none` (and for any value outside the vocabulary) —
 * an unprioritised project carries no priority chrome, so the axis reads as
 * signal rather than as a field that is always there.
 */
export function PriorityChip({ priority }: { priority: string }) {
	const appearance = priorityAppearance(priority);
	if (!appearance) return null;
	return (
		<Badge
			variant="ghost"
			className={cn("shrink-0", appearance.tint, appearance.fg)}
		>
			<FlagIcon />
			{appearance.label}
		</Badge>
	);
}
