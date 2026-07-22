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
import { DockIcon } from "@/features/dock/dock-icon";
import { DockPanel } from "@/features/dock/dock-panel";
import { EditorView } from "@/features/editor/editor-view";
import { ImportSheet } from "@/features/ingest/components/import-sheet";
import { OnboardingGate } from "@/features/onboarding/gate";
import { Palette } from "@/features/palette/palette";
import { PaletteDialogs } from "@/features/palette/palette-dialogs";
import { ProjectPeek } from "@/features/planner/peek/project-peek";
import { PlannerPage } from "@/features/planner/planner-page";
import { ProjectDetail } from "@/features/projects/detail/project-detail";
import { ProjectsList } from "@/features/projects/list/projects-list";
import { SettingsPage } from "@/features/settings/settings-page";
import { useBroadcastInvalidation } from "@/hooks/use-broadcast-invalidation";
import { useDeepLinkRouter } from "@/hooks/use-deep-link-router";
import { queryClient } from "@/lib/query-client";
import { applyTheme, storedTheme } from "@/lib/theme";
import { windowLabel } from "@/lib/window-label";
import { useUiStore } from "@/stores/ui";

function BroadcastBridge() {
	useBroadcastInvalidation();
	useDeepLinkRouter();
	return null;
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
	const editorBundlePath = useUiStore((s) => s.editorBundlePath);

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
					sidebar={<Sidebar dock={<DockIcon />} />}
					overlay={<DockPanel />}
				>
					{surface === "dashboard" && <Dashboard />}
					{surface === "planner" && <PlannerPage />}
					{surface === "projects" &&
						(editorBundlePath !== null ? (
							<EditorView bundlePath={editorBundlePath} />
						) : selectedProjectSlug !== null ? (
							<ProjectDetail slug={selectedProjectSlug} />
						) : (
							<ProjectsList />
						))}
					{surface === "settings" && <SettingsPage />}
				</AppShell>
				<Palette />
				<PaletteDialogs />
				<ProjectPeek />
				<ImportSheet />
			</OnboardingGate>
			<Toaster theme={dark ? "dark" : "light"} position="bottom-right" />
		</Providers>
	);
}
