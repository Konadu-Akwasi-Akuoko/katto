import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useDriveStatus } from "@/hooks/use-drive-status";

export function DriveCard() {
	const { data } = useDriveStatus();
	return (
		<Card>
			<CardHeader className="flex-row items-center justify-between">
				<CardTitle>Studio drive</CardTitle>
				{data !== undefined && data.path !== null && (
					<Badge variant={data.mounted ? "done" : "failed"}>
						<span className="dot" />
						{data.mounted ? "mounted" : "disconnected"}
					</Badge>
				)}
			</CardHeader>
			<CardContent className="flex flex-col gap-1">
				{data === undefined || data.path === null ? (
					<p className="text-fg-muted text-sm">No studio root configured.</p>
				) : (
					<>
						<p className="break-all font-mono text-fg-muted text-xs">
							{data.path}
						</p>
						{data.free_gb !== null && (
							<p className="tabular text-sm">{data.free_gb} GB free</p>
						)}
					</>
				)}
			</CardContent>
		</Card>
	);
}
