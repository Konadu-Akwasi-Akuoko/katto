import { ActiveJobs } from "@/features/dashboard/active-jobs";
import { ActivityFeed } from "@/features/dashboard/activity-feed";
import { DriveCard } from "@/features/dashboard/drive-card";

export function Dashboard() {
	return (
		<div className="flex flex-col gap-4 p-6">
			<h1 className="font-serif text-2xl">Dashboard</h1>
			<div className="grid items-start gap-4 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
				<ActivityFeed />
				<div className="flex flex-col gap-4">
					<ActiveJobs />
					<DriveCard />
				</div>
			</div>
		</div>
	);
}
