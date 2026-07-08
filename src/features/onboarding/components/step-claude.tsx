import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { claudeDetectKeys, completeOnboarding, detectClaude, storeKey } from "@/lib/ipc/onboarding";
import { settingsKeys } from "@/lib/ipc/settings";

export function StepClaude() {
	const queryClient = useQueryClient();
	const [anthropicKey, setAnthropicKey] = useState("");
	const [keyStored, setKeyStored] = useState(false);

	const detection = useQuery({
		queryKey: claudeDetectKeys.all,
		queryFn: detectClaude,
		staleTime: Number.POSITIVE_INFINITY,
	});
	const saveKey = useMutation({
		mutationFn: (key: string) => storeKey("anthropic", key),
		onSuccess: () => {
			setAnthropicKey("");
			setKeyStored(true);
		},
	});
	const finish = useMutation({
		mutationFn: completeOnboarding,
		onSuccess: () => queryClient.invalidateQueries({ queryKey: settingsKeys.all }),
	});

	return (
		<section className="flex flex-col gap-6">
			<div className="flex flex-col gap-2">
				<h1 className="font-serif text-3xl font-semibold">Claude</h1>
				<p className="text-fg-muted">
					Cut planning talks to Claude — through the claude CLI when it's installed, or
					the API with an Anthropic key. Neither is needed until the first rough cut.
				</p>
			</div>
			{detection.isPending && (
				<p className="text-sm text-fg-muted">Looking for claude on your PATH…</p>
			)}
			{detection.data != null && (
				<p className="text-sm">
					Found <span className="font-mono">{detection.data}</span>
				</p>
			)}
			{!detection.isPending && detection.data == null && (
				<div className="flex flex-col gap-3">
					<p className="text-sm text-fg-muted">
						claude isn't on your PATH. Paste an Anthropic key instead, or sort it out
						later in Settings.
					</p>
					{keyStored ? (
						<p className="text-sm">Key stored in your keychain.</p>
					) : (
						<div className="flex max-w-md items-end gap-2">
							<div className="flex flex-1 flex-col gap-1.5">
								<Label htmlFor="anthropic-key">Anthropic key</Label>
								<Input
									id="anthropic-key"
									type="password"
									value={anthropicKey}
									onChange={(e) => setAnthropicKey(e.target.value)}
									placeholder="sk-ant-…"
								/>
							</div>
							<Button
								variant="secondary"
								onClick={() => saveKey.mutate(anthropicKey)}
								disabled={anthropicKey.length === 0 || saveKey.isPending}
							>
								Save key
							</Button>
						</div>
					)}
				</div>
			)}
			<div>
				<Button onClick={() => finish.mutate()} disabled={finish.isPending}>
					Finish
				</Button>
			</div>
		</section>
	);
}
