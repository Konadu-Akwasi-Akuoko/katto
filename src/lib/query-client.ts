import { MutationCache, QueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { IpcError } from "@/lib/ipc/result";

const TITLES: Partial<Record<IpcError["kind"], string>> = {
	keychain: "Keychain write failed",
	onboarding: "Can't finish yet",
	db: "Database error",
	autostart: "Launch-at-login change failed",
};

/**
 * App-wide Query client. Every mutation failure funnels here — typed Rust
 * errors and infrastructure failures both land as a toast, so no feature
 * hand-rolls try/catch UI.
 */
export const queryClient = new QueryClient({
	mutationCache: new MutationCache({
		onError: (error) => {
			const title =
				error instanceof IpcError ? (TITLES[error.kind] ?? "Command failed") : "Command failed";
			toast.error(title, { description: error.message });
		},
	}),
});
