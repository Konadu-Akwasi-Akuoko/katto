import { Badge } from "@/components/ui/badge";

/**
 * The project's pipeline stage as a single dot-chip (idea → shooting → editing →
 * published). State is encoded once here — never also as a card rail.
 */
export function StatusChip({ status }: { status: string }) {
	return (
		<Badge variant="secondary" className="shrink-0 capitalize">
			<span className="dot" />
			{status}
		</Badge>
	);
}
