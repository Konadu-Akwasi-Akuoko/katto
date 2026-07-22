import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
	acceleratorFromKeyboardEvent,
	DEFAULT_CAPTURE_SHORTCUT,
	displayGlyphs,
} from "@/features/settings/model/accelerator";
import type { Settings } from "@/lib/ipc/settings";
import { setCaptureShortcut, settingsKeys } from "@/lib/ipc/settings";

/**
 * Rebind row for the quick-capture hotkey. "Rebind…" arms a recorder that takes
 * the next acceptable keydown (needs ⌘/⌃/⌥ + a key; Esc cancels); the combo is
 * validated and re-registered backend-side before it is persisted, so a rejected
 * combo leaves the old binding live and shown here — the failure itself surfaces
 * through the app-wide mutation toast.
 */
export function CaptureShortcutRow({ settings }: { settings: Settings }) {
	const queryClient = useQueryClient();
	const [recording, setRecording] = useState(false);
	const rebind = useMutation({
		mutationFn: setCaptureShortcut,
		onSuccess: () =>
			queryClient.invalidateQueries({ queryKey: settingsKeys.all }),
	});

	useEffect(() => {
		if (!recording) return;
		const onKeyDown = (event: KeyboardEvent) => {
			event.preventDefault();
			event.stopPropagation();
			if (event.code === "Escape") {
				setRecording(false);
				return;
			}
			const accel = acceleratorFromKeyboardEvent(event);
			if (accel === null) return;
			setRecording(false);
			rebind.mutate(accel);
		};
		window.addEventListener("keydown", onKeyDown, { capture: true });
		return () =>
			window.removeEventListener("keydown", onKeyDown, { capture: true });
	}, [recording, rebind.mutate]);

	return (
		<div className="flex items-center justify-between gap-4 text-sm">
			<Label>Quick capture shortcut</Label>
			<div className="flex items-center gap-2">
				{recording ? (
					<span className="text-fg-faint">Press shortcut — Esc cancels</span>
				) : (
					<span className="flex items-center gap-1">
						{displayGlyphs(settings.capture_shortcut).map((glyph) => (
							<Badge key={glyph} variant="outline">
								{glyph}
							</Badge>
						))}
					</span>
				)}
				{!recording &&
					settings.capture_shortcut !== DEFAULT_CAPTURE_SHORTCUT && (
						<Button
							variant="ghost"
							size="sm"
							disabled={rebind.isPending}
							onClick={() => rebind.mutate(DEFAULT_CAPTURE_SHORTCUT)}
						>
							Reset
						</Button>
					)}
				<Button
					variant="secondary"
					size="sm"
					disabled={rebind.isPending}
					onClick={() => setRecording((r) => !r)}
				>
					{recording ? "Cancel" : "Rebind…"}
				</Button>
			</div>
		</div>
	);
}
