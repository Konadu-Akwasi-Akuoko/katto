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
	// Queries and mutations run over Tauri IPC, not HTTP. The defaults are
	// network-shaped in two ways that blank the app: networkMode "online"
	// pauses fetching while the WebView reports offline (a Mac with no
	// internet), and the retry loop for a failing query pauses while the
	// window is hidden/unfocused — either way a failed getSettings never
	// settles and the onboarding gate renders nothing forever. IPC answers
	// or fails deterministically, so don't retry; recovery comes from
	// refetch-on-focus and broadcast invalidation.
	defaultOptions: {
		queries: { networkMode: "always", retry: false },
		mutations: { networkMode: "always" },
	},
	mutationCache: new MutationCache({
		onError: (error) => {
			const title =
				error instanceof IpcError
					? (TITLES[error.kind] ?? "Command failed")
					: "Command failed";
			toast.error(title, { description: error.message });
		},
	}),
});
