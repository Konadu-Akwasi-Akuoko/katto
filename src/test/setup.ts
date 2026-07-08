import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach } from "vitest";

// mockIPC leans on the Web Crypto API; polyfill for Node builds that don't
// expose it on the jsdom global.
if (!globalThis.crypto?.getRandomValues) {
	const { webcrypto } = await import("node:crypto");
	Object.defineProperty(globalThis, "crypto", { value: webcrypto });
}

afterEach(() => {
	cleanup();
	clearMocks();
});
