import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { pickRelocationFile, relocateSource } from "@/lib/ipc/editor";

/** m:ss display for the manifest duration. */
function durationCopy(seconds: number): string {
	const m = Math.floor(seconds / 60);
	const s = Math.round(seconds - m * 60);
	return `${m}:${String(s).padStart(2, "0")}`;
}

/**
 * SourceMissing flow: names the missing file, opens a picker filtered to it,
 * and heals the manifest only when the engine-side same-file check passes.
 * A mismatch names the failed check verbatim from the typed error.
 */
export function RelocateDialog({
	bundlePath,
	info,
	onClose,
	onRelocated,
}: {
	bundlePath: string;
	info: { expected_path: string; filename: string; duration_secs: number };
	onClose(): void;
	onRelocated(): void;
}) {
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const locate = async () => {
		setBusy(true);
		setError(null);
		try {
			const picked = await pickRelocationFile(info.filename);
			if (picked === null) return;
			await relocateSource(bundlePath, picked);
			onRelocated();
		} catch (e) {
			setError(e instanceof Error ? e.message : String(e));
		} finally {
			setBusy(false);
		}
	};

	return (
		<Dialog open onOpenChange={(open) => !open && onClose()}>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>Source video missing</DialogTitle>
					<DialogDescription>
						The bundle expects{" "}
						<span className="font-mono text-sm">{info.filename}</span> (
						{durationCopy(info.duration_secs)}) at{" "}
						<span className="font-mono text-sm">{info.expected_path}</span>.
						Point katto at the moved file — same name, same duration.
					</DialogDescription>
				</DialogHeader>
				{error !== null && <p className="text-sm text-failed">{error}</p>}
				<DialogFooter>
					<Button variant="ghost" onClick={onClose}>
						Cancel
					</Button>
					<Button disabled={busy} onClick={() => void locate()}>
						Locate file…
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
