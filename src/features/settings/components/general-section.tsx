import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { CaptureShortcutRow } from "@/features/settings/components/capture-shortcut-row";
import { useAutostart } from "@/features/settings/hooks/use-autostart";
import type { Settings } from "@/lib/ipc/settings";
import { patchSettings, settingsKeys } from "@/lib/ipc/settings";

export function GeneralSection({ settings }: { settings: Settings }) {
	const queryClient = useQueryClient();
	const autostart = useAutostart();
	const patch = useMutation({
		mutationFn: patchSettings,
		onSuccess: () =>
			queryClient.invalidateQueries({ queryKey: settingsKeys.all }),
	});

	return (
		<section className="flex flex-col gap-4">
			<h2 className="font-serif text-lg font-semibold">General</h2>
			<div className="flex items-center justify-between text-sm">
				<Label htmlFor="autostart">Launch at login</Label>
				<Switch
					id="autostart"
					checked={autostart.state.data ?? false}
					onCheckedChange={(v) => autostart.toggle.mutate(v)}
					disabled={autostart.state.isPending || autostart.toggle.isPending}
				/>
			</div>
			<div className="flex items-center justify-between gap-4 text-sm">
				<Label htmlFor="nle">Editor for exports</Label>
				<Select
					value={settings.default_nle ?? undefined}
					onValueChange={(nle) => patch.mutate({ default_nle: nle })}
				>
					<SelectTrigger id="nle" className="w-56">
						<SelectValue placeholder="Not set — chosen at first export" />
					</SelectTrigger>
					<SelectContent>
						<SelectItem value="fcp">Final Cut Pro</SelectItem>
						<SelectItem value="resolve">DaVinci Resolve</SelectItem>
					</SelectContent>
				</Select>
			</div>
			<CaptureShortcutRow settings={settings} />
		</section>
	);
}
