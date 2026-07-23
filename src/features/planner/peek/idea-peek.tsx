import { skipToken, useQuery } from "@tanstack/react-query";
import { IdeaDetailModal } from "@/features/planner/backlog/idea-detail-modal";
import { getIdea, ideasKeys } from "@/lib/ipc/ideas";
import { useUiStore } from "@/stores/ui";

/**
 * Shared host for the idea detail modal, mounted once at the shell so any surface
 * (the Backlog list, the calendar's backlog markers) can open it by id. Mirrors
 * {@link ProjectPeek}: the store holds the id, this fetches the row.
 */
export function IdeaPeek() {
	const openIdeaId = useUiStore((s) => s.openIdeaId);
	const closeIdea = useUiStore((s) => s.closeIdea);

	const { data } = useQuery({
		queryKey: ideasKeys.detail(openIdeaId ?? ""),
		queryFn: openIdeaId === null ? skipToken : () => getIdea(openIdeaId),
		enabled: openIdeaId !== null,
	});

	if (openIdeaId === null || !data) return null;
	return <IdeaDetailModal idea={data} onClose={closeIdea} />;
}
