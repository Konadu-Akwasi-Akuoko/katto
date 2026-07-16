import { QueryClientProvider, useQuery } from "@tanstack/react-query";
import { render, waitFor } from "@testing-library/react";
import { toast } from "sonner";
import { describe, expect, it, vi } from "vitest";
import { IpcError } from "@/lib/ipc/result";
import { queryClient } from "@/lib/query-client";

vi.mock("sonner", () => ({ toast: { error: vi.fn() } }));

function Boom({ error }: { error: Error }) {
	useQuery({
		queryKey: ["boom", error.message],
		queryFn: () => Promise.reject(error),
	});
	return null;
}

describe("query-client", () => {
	it("toasts a failing query with the error kind's title", async () => {
		render(
			<QueryClientProvider client={queryClient}>
				<Boom error={new IpcError({ kind: "db", message: "disk I/O error" })} />
			</QueryClientProvider>,
		);
		await waitFor(() =>
			expect(toast.error).toHaveBeenCalledWith(
				"Database error",
				expect.objectContaining({ description: "disk I/O error" }),
			),
		);
	});

	it("names the unmounted studio root rather than 'Command failed'", async () => {
		render(
			<QueryClientProvider client={queryClient}>
				<Boom
					error={
						new IpcError({
							kind: "studio_root_unmounted",
							message: "studio root is not mounted: /Volumes/Studio",
						})
					}
				/>
			</QueryClientProvider>,
		);
		await waitFor(() =>
			expect(toast.error).toHaveBeenCalledWith(
				"Studio drive isn't mounted",
				expect.anything(),
			),
		);
	});

	it("toasts a non-IPC query failure as a generic load failure", async () => {
		render(
			<QueryClientProvider client={queryClient}>
				<Boom error={new Error("window not reachable")} />
			</QueryClientProvider>,
		);
		await waitFor(() =>
			expect(toast.error).toHaveBeenCalledWith(
				"Couldn't load",
				expect.objectContaining({ description: "window not reachable" }),
			),
		);
	});
});
