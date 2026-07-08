import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { KeyService } from "@/lib/ipc/onboarding";
import { storeKey } from "@/lib/ipc/onboarding";
import type { Settings } from "@/lib/ipc/settings";
import { settingsKeys } from "@/lib/ipc/settings";
import { cn } from "@/lib/utils";

function KeyRow({
	service,
	label,
	present,
}: {
	service: KeyService;
	label: string;
	present: boolean;
}) {
	const queryClient = useQueryClient();
	const [value, setValue] = useState("");
	const save = useMutation({
		mutationFn: (key: string) => storeKey(service, key),
		onSuccess: () => {
			setValue("");
			queryClient.invalidateQueries({ queryKey: settingsKeys.all });
		},
	});
	const inputId = `${service}-key`;

	return (
		<div className="flex flex-col gap-1.5">
			<div className="flex items-center gap-2">
				<Label htmlFor={inputId}>{`${label} API key`}</Label>
				<span className="flex items-center gap-1.5 text-xs text-fg-muted">
					<span
						className={cn(
							"size-1.5 rounded-full",
							present ? "bg-done" : "bg-fg-faint",
						)}
					/>
					{present ? "in keychain" : "not set"}
				</span>
			</div>
			<div className="flex max-w-md gap-2">
				<Input
					id={inputId}
					type="password"
					value={value}
					onChange={(e) => setValue(e.target.value)}
					placeholder={present ? "Replace key" : "Paste key"}
				/>
				<Button
					variant="secondary"
					onClick={() => save.mutate(value)}
					disabled={value.length === 0 || save.isPending}
				>
					{`Save ${label} key`}
				</Button>
			</div>
		</div>
	);
}

export function KeysSection({ settings }: { settings: Settings }) {
	return (
		<section className="flex flex-col gap-4">
			<h2 className="font-serif text-lg font-semibold">Keys</h2>
			<p className="text-sm text-fg-muted">
				Keys live in the macOS keychain. katto never shows a stored key again.
			</p>
			<KeyRow
				service="elevenlabs"
				label="ElevenLabs"
				present={settings.keys_present.elevenlabs}
			/>
			<KeyRow
				service="anthropic"
				label="Anthropic"
				present={settings.keys_present.anthropic}
			/>
		</section>
	);
}
