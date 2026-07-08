import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { storeKey } from "@/lib/ipc/onboarding";

export function StepKey({ onDone }: { onDone: () => void }) {
	const [value, setValue] = useState("");
	const [stored, setStored] = useState(false);

	const save = useMutation({
		mutationFn: (key: string) => storeKey("elevenlabs", key),
		onSuccess: () => {
			setValue("");
			setStored(true);
		},
	});

	return (
		<section className="flex flex-col gap-6">
			<div className="flex flex-col gap-2">
				<h1 className="font-serif text-3xl font-semibold">ElevenLabs key</h1>
				<p className="text-fg-muted">
					Transcription runs through ElevenLabs Scribe. The key goes straight into the
					macOS keychain — katto never shows it again. You can also add it later in
					Settings.
				</p>
			</div>
			{stored ? (
				<p className="text-sm">Key stored in your keychain.</p>
			) : (
				<div className="flex max-w-md flex-col gap-1.5">
					<Label htmlFor="elevenlabs-key">API key</Label>
					<div className="flex gap-2">
						<Input
							id="elevenlabs-key"
							type="password"
							value={value}
							onChange={(e) => setValue(e.target.value)}
							placeholder="xi-…"
						/>
						<Button
							variant="secondary"
							onClick={() => save.mutate(value)}
							disabled={value.length === 0 || save.isPending}
						>
							Save key
						</Button>
					</div>
				</div>
			)}
			<div>
				<Button onClick={onDone}>{stored ? "Continue" : "Skip for now"}</Button>
			</div>
		</section>
	);
}
