import type { ReactNode } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { BacklogView } from "@/features/planner/backlog/backlog-view";

export function PlannerPage() {
	return (
		<div className="flex h-full min-h-0 flex-col gap-4 p-6">
			<h1 className="font-serif text-2xl">Planner</h1>
			<Tabs defaultValue="backlog" className="flex min-h-0 flex-1 flex-col">
				<TabsList variant="line" className="self-start">
					<TabsTrigger value="board">Board</TabsTrigger>
					<TabsTrigger value="calendar">Calendar</TabsTrigger>
					<TabsTrigger value="backlog">Backlog</TabsTrigger>
				</TabsList>
				<TabsContent value="board" className="min-h-0">
					<Placeholder>
						The board arrives with drag-to-status. For now, triage the backlog.
					</Placeholder>
				</TabsContent>
				<TabsContent value="calendar" className="min-h-0">
					<Placeholder>
						The calendar arrives next, with shoot and publish dates as chips.
					</Placeholder>
				</TabsContent>
				<TabsContent value="backlog" className="min-h-0">
					<BacklogView />
				</TabsContent>
			</Tabs>
		</div>
	);
}

function Placeholder({ children }: { children: ReactNode }) {
	return <p className="text-sm text-fg-muted">{children}</p>;
}
