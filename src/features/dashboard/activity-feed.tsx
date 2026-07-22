import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { eventLine, relativeTime } from "@/features/dashboard/model/format";
import { eventsKeys, listEvents } from "@/lib/ipc/events";

const FEED_LIMIT = 50;

export function ActivityFeed() {
	const { data: events } = useQuery({
		queryKey: eventsKeys.all,
		queryFn: () => listEvents(FEED_LIMIT),
	});
	const now = new Date();
	return (
		<Card>
			<CardHeader>
				<CardTitle>Activity</CardTitle>
			</CardHeader>
			<CardContent>
				{events === undefined || events.length === 0 ? (
					<p className="text-fg-muted text-sm">
						Quiet so far. Everything katto does lands here — ingests, cuts,
						exports, downloads.
					</p>
				) : (
					<ScrollArea className="max-h-[60dvh]">
						<ul className="flex flex-col gap-2 pr-3">
							{events.map((event) => (
								<li
									key={event.id}
									className="flex items-baseline justify-between gap-3"
								>
									<span className="text-sm">{eventLine(event)}</span>
									<time className="tabular whitespace-nowrap text-fg-faint text-xs">
										{relativeTime(event.ts, now)}
									</time>
								</li>
							))}
						</ul>
					</ScrollArea>
				)}
			</CardContent>
		</Card>
	);
}
