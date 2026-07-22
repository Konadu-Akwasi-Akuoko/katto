import type { VfxEffect } from "@/lib/ipc/bindings.gen";
import { commands } from "@/lib/ipc/bindings.gen";
import { unwrap } from "@/lib/ipc/result";

export type { VfxEffect };

export const vfxKeys = {
	byProject: (slug: string) => ["vfx", slug] as const,
};

/** Every effect folder of a project (folders are truth). */
export const listVfxEffects = (slug: string): Promise<VfxEffect[]> =>
	unwrap(commands.listVfxEffects(slug));

/**
 * Scaffold `assets/vfx/<slug>/` and open a dock session in it; resolves to
 * the session id so the caller can focus the dock.
 */
export const createVfxEffect = (slug: string, name: string): Promise<string> =>
	unwrap(commands.createVfxEffect(slug, name));
