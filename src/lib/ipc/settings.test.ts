import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { getSettings } from "@/lib/ipc/settings";
import { settingsFixture } from "@/test/fixtures/settings";

describe("settings ipc wrapper", () => {
	it("unwraps a successful get_settings invoke", async () => {
		mockIPC((cmd) => {
			if (cmd === "get_settings") {
				return settingsFixture;
			}
			throw new Error(`unexpected command ${cmd}`);
		});

		await expect(getSettings()).resolves.toMatchObject({
			onboarding_complete: false,
			keys_present: { elevenlabs: false, anthropic: false },
		});
	});
});
