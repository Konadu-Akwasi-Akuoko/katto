import { Separator } from "@/components/ui/separator";
import { ClaudeSection } from "@/features/settings/components/claude-section";
import { GeneralSection } from "@/features/settings/components/general-section";
import { KeysSection } from "@/features/settings/components/keys-section";
import { StudioRootSection } from "@/features/settings/components/studio-root-section";
import { useSettings } from "@/hooks/use-settings";

export function SettingsPage() {
	const settings = useSettings();
	if (settings.isPending || settings.isError) return null;

	return (
		<div className="mx-auto flex w-full max-w-2xl flex-col gap-6 p-6">
			<h1 className="font-serif text-2xl font-semibold">Settings</h1>
			<GeneralSection settings={settings.data} />
			<Separator />
			<StudioRootSection settings={settings.data} />
			<Separator />
			<KeysSection settings={settings.data} />
			<Separator />
			<ClaudeSection settings={settings.data} />
		</div>
	);
}
