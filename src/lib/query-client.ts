import { MutationCache, QueryCache, QueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { IpcError } from "@/lib/ipc/result";

const TITLES: Partial<Record<IpcError["kind"], string>> = {
	keychain: "Keychain write failed",
	onboarding: "Can't finish yet",
	db: "Database error",
	autostart: "Launch-at-login change failed",
	studio_root_unmounted: "Studio drive isn't mounted",
	io: "Couldn't reach that folder",
	shortcut_invalid: "Shortcut not accepted",
	shortcut_unavailable: "Shortcut already in use",
};

/**
 * App-wide Query client. Every query and mutation failure funnels here — typed
 * Rust errors and infrastructure failures both land as a toast, so no feature
 * hand-rolls try/catch UI and nothing fails silently. Views still owe an
 * explicit error state where an empty result would otherwise look plausible: a
 * board rendering "0" for an unreachable backend is a lie a toast doesn't undo.
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
	queryCache: new QueryCache({
		onError: (error) => {
			// One toast per failure kind: a backend that is down fails every
			// mounted query at once, and sonner dedupes on a stable id.
			const kind = error instanceof IpcError ? error.kind : "unknown";
			const title =
				error instanceof IpcError
					? (TITLES[error.kind] ?? "Couldn't load")
					: "Couldn't load";
			toast.error(title, {
				id: `query-error-${kind}`,
				description: error.message,
			});
		},
	}),
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
