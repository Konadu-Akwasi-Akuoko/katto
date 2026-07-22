import { useQuery, useQueryClient } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { useEffect } from "react";

import { useIngestSheetStore } from "@/stores/ingest-sheet";
import { onCardDetected, onCardRemoved } from "@/lib/ipc/broadcast";
import { cardOffer, ingestKeys } from "@/lib/ipc/ingest";

/**
 * The current card offer, kept live by the card broadcasts: detection refetches
 * and opens the import sheet; removal refetches so a stale offer disappears.
 */
export function useCardOffer() {
	const queryClient = useQueryClient();
	const setOpen = useIngestSheetStore((s) => s.setOpen);

	useEffect(() => {
		if (!isTauri()) return;
		const detected = onCardDetected(() => {
			void queryClient.invalidateQueries({ queryKey: ingestKeys.offer() });
			setOpen(true);
		});
		const removed = onCardRemoved(() => {
			void queryClient.invalidateQueries({ queryKey: ingestKeys.offer() });
		});
		return () => {
			void detected.then((unlisten) => unlisten());
			void removed.then((unlisten) => unlisten());
		};
	}, [queryClient, setOpen]);

	return useQuery({ queryKey: ingestKeys.offer(), queryFn: cardOffer });
}
