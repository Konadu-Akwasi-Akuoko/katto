import { commands } from "@/lib/ipc/bindings.gen";
import type { KeyService, RootCheck } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { KeyService, RootCheck };

/** Open the native folder picker; null when the user cancels. */
export const pickStudioRoot = (): Promise<RootCheck | null> =>
	unwrap(commands.pickStudioRoot());

/** Write a credential to the macOS keychain (write-only; never read back). */
export const storeKey = (service: KeyService, value: string): Promise<null> =>
	unwrap(commands.storeKey(service, value));

/** Whether a credential exists for the service. */
export const keyPresent = (service: KeyService): Promise<boolean> =>
	unwrap(commands.keyPresent(service));

/** Locate `claude` on the login-shell PATH; caches a found path in settings. */
export const detectClaude = (): Promise<string | null> =>
	unwrap(commands.detectClaude());

/** Flip the onboarding flag (requires a saved studio root). */
export const completeOnboarding = (): Promise<null> =>
	unwrap(commands.completeOnboarding());

/** TanStack Query key for the claude-detection probe. */
export const claudeDetectKeys = { all: ["claude-detect"] as const };
