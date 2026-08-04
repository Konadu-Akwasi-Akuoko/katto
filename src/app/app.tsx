import { useEffect, useState } from "react";
import { registerAppCommands } from "@/app/commands";
import { Providers } from "@/app/providers";
import { AppShell } from "@/components/layout/app-shell";
import { DriveBanner } from "@/components/layout/drive-banner";
import { Sidebar } from "@/components/layout/sidebar";
import { Titlebar } from "@/components/layout/titlebar";
import { Toaster } from "@/components/ui/sonner";
import { BrowserSurface } from "@/features/browser/browser-surface";
import { DownloadsBridge } from "@/features/browser/downloads-bridge";
import { CaptureForm } from "@/features/capture/capture-form";
import { Dashboard } from "@/features/dashboard/dashboard";
import { DockIcon } from "@/features/dock/dock-icon";
import { DockPanel } from "@/features/dock/dock-panel";
import { EditorView } from "@/features/editor/editor-view";
import { ImportSheet } from "@/features/ingest/components/import-sheet";
import { OnboardingGate } from "@/features/onboarding/gate";
import { Palette } from "@/features/palette/palette";
import { PaletteDialogs } from "@/features/palette/palette-dialogs";
import { IdeaPeek } from "@/features/planner/peek/idea-peek";
import { ProjectPeek } from "@/features/planner/peek/project-peek";
import { PlannerPage } from "@/features/planner/planner-page";
import { ProjectDetail } from "@/features/projects/detail/project-detail";
import { ProjectsList } from "@/features/projects/list/projects-list";
import { SettingsPage } from "@/features/settings/settings-page";
import { useBroadcastInvalidation } from "@/hooks/use-broadcast-invalidation";
import { useDeepLinkRouter } from "@/hooks/use-deep-link-router";
import { browserSetVisible } from "@/lib/ipc/browser";
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
		// The host's visibility flag outlives this webview's JS context: a reload
		// tears React down without running BrowserSurface's unmount cleanup, so
		// the child webview stays shown at its last rect, painted over whichever
		// surface the fresh UI boots on. Nothing is mounted yet at this point, so
		// hidden is the truth.
		void browserSetVisible(false);
	}, []);

	function toggleTheme() {
		const next = dark ? "light" : "dark";
		applyTheme(next);
		setDark(next === "dark");
	}

	return (
		<Providers>
			<BroadcastBridge />
			<DownloadsBridge />
			<OnboardingGate>
				<AppShell
					titlebar={
						<Titlebar
							dark={dark}
							onToggleTheme={toggleTheme}
							dock={<DockIcon compact />}
						/>
					}
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
					{surface === "browser" && <BrowserSurface />}
					{surface === "settings" && <SettingsPage />}
				</AppShell>
				<Palette />
				<PaletteDialogs />
				<ProjectPeek />
				<IdeaPeek />
				<ImportSheet />
			</OnboardingGate>
			<Toaster theme={dark ? "dark" : "light"} position="bottom-right" />
		</Providers>
	);
}
