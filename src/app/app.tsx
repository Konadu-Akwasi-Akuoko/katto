import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { registerAppCommands } from "@/app/commands";
import { Providers } from "@/app/providers";
import { AppShell } from "@/components/layout/app-shell";
import { DriveBanner } from "@/components/layout/drive-banner";
import { Sidebar } from "@/components/layout/sidebar";
import { Titlebar } from "@/components/layout/titlebar";
import { Toaster } from "@/components/ui/sonner";
import { CaptureForm } from "@/features/capture/capture-form";
import { Dashboard } from "@/features/dashboard/dashboard";
import { OnboardingGate } from "@/features/onboarding/gate";
import { Palette } from "@/features/palette/palette";
import { PlannerPage } from "@/features/planner/planner-page";
import { ProjectDetail } from "@/features/projects/detail/project-detail";
import { ProjectsList } from "@/features/projects/list/projects-list";
import { SettingsPage } from "@/features/settings/settings-page";
import { useBroadcastInvalidation } from "@/hooks/use-broadcast-invalidation";
import { useDeepLinkRouter } from "@/hooks/use-deep-link-router";
import { queryClient } from "@/lib/query-client";
import { applyTheme, storedTheme } from "@/lib/theme";
import { useUiStore } from "@/stores/ui";

function BroadcastBridge() {
	useBroadcastInvalidation();
	useDeepLinkRouter();
	return null;
}

/** The window label, defaulting to `main` when the Tauri context is absent (tests, plain browser). */
function windowLabel(): string {
	try {
		return getCurrentWindow().label;
	} catch {
		return "main";
	}
}

export default function App() {
	const [dark] = useState(() => storedTheme() === "dark");

	if (windowLabel() === "capture") {
		return (
			<Providers>
				<CaptureForm />
				<Toaster theme={dark ? "dark" : "light"} position="bottom-right" />
			</Providers>
		);
	}

	return <MainApp />;
}

function MainApp() {
	const [dark, setDark] = useState(() => storedTheme() === "dark");
	const surface = useUiStore((s) => s.surface);
	const selectedProjectSlug = useUiStore((s) => s.selectedProjectSlug);

	useEffect(() => {
		registerAppCommands(queryClient);
	}, []);

	function toggleTheme() {
		const next = dark ? "light" : "dark";
		applyTheme(next);
		setDark(next === "dark");
	}

	return (
		<Providers>
			<BroadcastBridge />
			<OnboardingGate>
				<AppShell
					titlebar={<Titlebar dark={dark} onToggleTheme={toggleTheme} />}
					banner={<DriveBanner />}
					sidebar={<Sidebar />}
				>
					{surface === "dashboard" && <Dashboard />}
					{surface === "planner" && <PlannerPage />}
					{surface === "projects" &&
						(selectedProjectSlug !== null ? (
							<ProjectDetail slug={selectedProjectSlug} />
						) : (
							<ProjectsList />
						))}
					{surface === "settings" && <SettingsPage />}
				</AppShell>
				<Palette />
			</OnboardingGate>
			<Toaster theme={dark ? "dark" : "light"} position="bottom-right" />
		</Providers>
	);
}
