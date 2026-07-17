import { useState } from "react";
import { StepClaude } from "@/features/onboarding/components/step-claude";
import { StepKey } from "@/features/onboarding/components/step-key";
import { StepRoot } from "@/features/onboarding/components/step-root";

const STEP_COUNT = 3;

/** First-run wizard. Only the studio root is required; keys can land later in Settings. */
export function OnboardingWizard() {
	const [step, setStep] = useState(0);
	return (
		<main className="flex h-dvh flex-col justify-center bg-bg px-16">
			<div className="flex w-full max-w-xl flex-col gap-4">
				<p className="text-sm text-fg-faint tabular-nums">{`Step ${step + 1} of ${STEP_COUNT}`}</p>
				{step === 0 && <StepRoot onDone={() => setStep(1)} />}
				{step === 1 && <StepKey onDone={() => setStep(2)} />}
				{step === 2 && <StepClaude />}
			</div>
		</main>
	);
}
