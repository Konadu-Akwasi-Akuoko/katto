import type { ReactNode } from "react";
import { useSettings } from "@/hooks/use-settings";
import { OnboardingWizard } from "@/features/onboarding/wizard";

/**
 * Holds the app behind the wizard until onboarding has completed once. A
 * settings load error falls through to the app rather than trapping the owner
 * in the wizard — a broken DB is already fatal at backend bootstrap.
 */
export function OnboardingGate({ children }: { children: ReactNode }) {
	const settings = useSettings();
	if (settings.isPending) return null;
	if (settings.isError || settings.data.onboarding_complete) return <>{children}</>;
	return <OnboardingWizard />;
}
