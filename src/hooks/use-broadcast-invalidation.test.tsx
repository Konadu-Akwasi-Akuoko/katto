import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { useBroadcastInvalidation } from "@/hooks/use-broadcast-invalidation";
import { driveKeys } from "@/lib/ipc/drive";
import { eventsKeys } from "@/lib/ipc/events";
import { ideasKeys } from "@/lib/ipc/ideas";
import { jobsKeys } from "@/lib/ipc/jobs";

describe("useBroadcastInvalidation", () => {
	it("maps each backend broadcast to its query-key invalidation", async () => {
		mockIPC(() => undefined, { shouldMockEvents: true });
		const client = new QueryClient();
		const invalidate = vi.spyOn(client, "invalidateQueries");

		function wrapper({ children }: { children: ReactNode }) {
			return (
				<QueryClientProvider client={client}>{children}</QueryClientProvider>
			);
		}
		renderHook(() => useBroadcastInvalidation(), { wrapper });

		// The listeners attach in an effect whose `listen()` calls resolve on a
		// microtask; cross a macrotask boundary so they are registered before we emit.
		await new Promise((resolve) => setTimeout(resolve, 0));

		await emit("events-appended");
		await emit("jobs-changed");
		await emit("ideas-changed");
		await emit("drive-status-changed");

		await waitFor(() => {
			expect(invalidate).toHaveBeenCalledWith({ queryKey: eventsKeys.all });
			expect(invalidate).toHaveBeenCalledWith({ queryKey: jobsKeys.all });
			expect(invalidate).toHaveBeenCalledWith({ queryKey: ideasKeys.all });
			expect(invalidate).toHaveBeenCalledWith({ queryKey: driveKeys.status });
		});
	});
});
