import type { RootCheck } from "@/lib/ipc/onboarding";

/** Advisory copy for a picked studio root. Empty when nothing needs saying. */
export function rootWarnings(check: RootCheck): string[] {
	const warnings: string[] = [];
	if (!check.writable) {
		warnings.push("katto can't write to this folder. Pick one you own.");
	}
	if (check.on_boot_volume) {
		warnings.push(
			"This folder is on your Mac's internal drive. Camera footage fills it fast — an external SSD is the comfortable choice.",
		);
	}
	if (check.low_free_space) {
		const free = check.free_gb === null ? "under 100" : String(check.free_gb);
		warnings.push(`Only ${free} GB free here. One shoot day can eat that.`);
	}
	return warnings;
}

/** The wizard's only hard requirement: a picked root katto can write to. */
export function canContinue(check: RootCheck | null): boolean {
	// biome-ignore lint/complexity/useOptionalChain: check?.writable would return boolean | undefined, breaking the declared boolean return (null case must be false, not undefined)
	return check !== null && check.writable;
}
