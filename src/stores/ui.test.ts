import { beforeEach, describe, expect, it } from "vitest";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
	useUiStore.setState({ peekSlug: null });
});

describe("ui store peek channel", () => {
	it("opens the peek for a slug", () => {
		useUiStore.getState().openPeek("nvme-deep-dive-2026-07-01");
		expect(useUiStore.getState().peekSlug).toBe("nvme-deep-dive-2026-07-01");
	});

	it("closes the peek", () => {
		useUiStore.getState().openPeek("x");
		useUiStore.getState().closePeek();
		expect(useUiStore.getState().peekSlug).toBeNull();
	});
});
