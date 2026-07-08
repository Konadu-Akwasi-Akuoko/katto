import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { getDriveStatus } from "@/lib/ipc/drive";

describe("getDriveStatus", () => {
	it("unwraps the drive status snapshot", async () => {
		mockIPC((cmd) => {
			if (cmd === "get_drive_status") {
				return { mounted: false, path: "/Volumes/Studio", free_gb: null };
			}
			throw new Error(`unexpected command: ${cmd}`);
		});

		await expect(getDriveStatus()).resolves.toMatchObject({
			mounted: false,
			path: "/Volumes/Studio",
		});
	});
});
