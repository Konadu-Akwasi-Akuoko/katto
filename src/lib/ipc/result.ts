import type { Error as IpcErrorPayload } from "@/lib/ipc/bindings.gen";

export type { IpcErrorPayload };

/**
 * A rejected command, carrying the tagged Rust error so callers (and the
 * TanStack Query error handlers) can switch on `kind`. `source_missing` is
 * the one kind whose wire `message` is structured; its fields surface on
 * {@link IpcError.sourceMissing} and `message` is re-derived for display.
 */
export class IpcError extends Error {
	readonly kind: IpcErrorPayload["kind"];
	readonly sourceMissing?: {
		expected_path: string;
		filename: string;
		duration_secs: number;
	};

	constructor(payload: IpcErrorPayload) {
		super(
			payload.kind === "source_missing"
				? `source missing: expected ${payload.message.expected_path}`
				: payload.message,
		);
		this.name = "IpcError";
		this.kind = payload.kind;
		if (payload.kind === "source_missing") {
			this.sourceMissing = payload.message;
		}
	}
}

type CommandResult<T> =
	| { status: "ok"; data: T }
	| { status: "error"; error: IpcErrorPayload };

/**
 * Unwrap a tauri-specta command result, returning its data or throwing
 * {@link IpcError}. Every `src/lib/ipc/<domain>.ts` wrapper funnels through this
 * so feature code never sees the `{ status }` envelope.
 *
 * Note: only Rust `Err` returns become {@link IpcError}. Infrastructure/JS-level
 * invoke failures reject with a bare `Error` (the generated runtime re-throws
 * those). The TanStack Query error handlers (frontend-toolchain slice) must
 * handle both shapes, not assume every rejection has a `.kind`.
 */
export async function unwrap<T>(result: Promise<CommandResult<T>>): Promise<T> {
	const settled = await result;
	if (settled.status === "error") throw new IpcError(settled.error);
	return settled.data;
}
