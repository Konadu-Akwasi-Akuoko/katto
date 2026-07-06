import type { Error as IpcErrorPayload } from "@/lib/ipc/bindings.gen";

export type { IpcErrorPayload };

/**
 * A rejected command, carrying the tagged Rust error so callers (and the
 * TanStack Query error handlers) can switch on `kind`.
 */
export class IpcError extends Error {
	readonly kind: IpcErrorPayload["kind"];

	constructor(payload: IpcErrorPayload) {
		super(payload.message);
		this.name = "IpcError";
		this.kind = payload.kind;
	}
}

type CommandResult<T> =
	| { status: "ok"; data: T }
	| { status: "error"; error: IpcErrorPayload };

/**
 * Unwrap a tauri-specta command result, returning its data or throwing
 * {@link IpcError}. Every `src/lib/ipc/<domain>.ts` wrapper funnels through this
 * so feature code never sees the `{ status }` envelope.
 */
export async function unwrap<T>(result: Promise<CommandResult<T>>): Promise<T> {
	const settled = await result;
	if (settled.status === "error") throw new IpcError(settled.error);
	return settled.data;
}
