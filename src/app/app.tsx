import { useEffect, useState } from "react";
import { registerAppCommands } from "@/app/commands";
import { Providers } from "@/app/providers";
import { AppShell } from "@/components/layout/app-shell";
import { Sidebar } from "@/components/layout/sidebar";
import { Titlebar } from "@/components/layout/titlebar";
import { Toaster } from "@/components/ui/sonner";
import { Dashboard } from "@/features/dashboard/dashboard";
import { OnboardingGate } from "@/features/onboarding/gate";
import { Palette } from "@/features/palette/palette";
import { PlannerStub } from "@/features/planner/planner-stub";
import { ProjectsStub } from "@/features/projects/projects-stub";
import { SettingsPage } from "@/features/settings/settings-page";
import { queryClient } from "@/lib/query-client";
import { applyTheme, storedTheme } from "@/lib/theme";
import { useUiStore } from "@/stores/ui";

export default function App() {
	const [dark, setDark] = useState(() => storedTheme() === "dark");
	const surface = useUiStore((s) => s.surface);

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
			<OnboardingGate>
				<AppShell
					titlebar={<Titlebar dark={dark} onToggleTheme={toggleTheme} />}
					sidebar={<Sidebar />}
				>
					{surface === "dashboard" && <Dashboard />}
					{surface === "planner" && <PlannerStub />}
					{surface === "projects" && <ProjectsStub />}
					{surface === "settings" && <SettingsPage />}
				</AppShell>
				<Palette />
			</OnboardingGate>
			<Toaster theme={dark ? "dark" : "light"} position="bottom-right" />
		</Providers>
	);
}
