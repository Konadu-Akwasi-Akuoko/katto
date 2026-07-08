import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { getSettings } from "@/lib/ipc/settings";

describe("settings ipc wrapper", () => {
	it("unwraps a successful get_settings invoke", async () => {
		mockIPC((cmd) => {
			if (cmd === "get_settings") {
				return {
					studio_root: null,
					default_nle: null,
					idle_reap_minutes: 10,
					onboarding_complete: false,
					claude_path: null,
					keys_present: { elevenlabs: false, anthropic: false },
				};
			}
			throw new Error(`unexpected command ${cmd}`);
		});

		await expect(getSettings()).resolves.toMatchObject({
			onboarding_complete: false,
			keys_present: { elevenlabs: false, anthropic: false },
		});
	});
});
