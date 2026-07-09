import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// mockIPC leans on the Web Crypto API; polyfill for Node builds that don't
// expose it on the jsdom global.
if (!globalThis.crypto?.getRandomValues) {
	const { webcrypto } = await import("node:crypto");
	Object.defineProperty(globalThis, "crypto", { value: webcrypto });
}

// Tests run against mockIPC, i.e. "inside Tauri"; isTauri() checks this
// global, which the tauri mocks module does not set.
Object.defineProperty(globalThis, "isTauri", {
	value: true,
	configurable: true,
});

// cmdk scrolls the selected palette item into view; jsdom has no layout.
if (!Element.prototype.scrollIntoView) {
	Element.prototype.scrollIntoView = () => {};
}

// cmdk observes its list for resize; jsdom ships no ResizeObserver.
if (!globalThis.ResizeObserver) {
	globalThis.ResizeObserver = class {
		observe() {}
		unobserve() {}
		disconnect() {}
	};
}

afterEach(async () => {
	cleanup();
	// Effect teardown that unlistens from tauri events defers the actual
	// unlisten to a microtask (`subscription.then(unlisten)`). Cross one
	// macrotask boundary so that chain settles while the mock IPC internals
	// are still installed, before clearMocks tears them down.
	await new Promise((resolve) => setTimeout(resolve, 0));
	clearMocks();
});
