import type { Settings } from "@/lib/ipc/settings";

/** Baseline Settings row for tests; spread-override per scenario. */
export const settingsFixture: Settings = {
	studio_root: null,
	default_nle: null,
	idle_reap_minutes: 10,
	onboarding_complete: false,
	claude_path: null,
	capture_shortcut: "alt+cmd+k",
	planner_model: "claude-sonnet-4-6",
	discovery_enabled: false,
	hyperframes_path: null,
	dock_planning: true,
	keys_present: { elevenlabs: false, anthropic: false },
};
