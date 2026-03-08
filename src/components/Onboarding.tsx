import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

import type { Step } from './onboarding/types';
import { WelcomeStep } from './onboarding/WelcomeStep';
import { TasteTestStep } from './onboarding/TasteTestStep';
import { QuickSetupStep } from './onboarding/QuickSetupStep';
import { CalibrationStep } from './onboarding/CalibrationStep';

interface OnboardingProps {
  onComplete: () => void;
}

const steps: Step[] = ['welcome', 'taste', 'setup', 'calibrate'];
const WIZARD_STEP_KEY = '4da-onboarding-wizard-step';

function getPersistedStep(): Step {
  try {
    const stored = localStorage.getItem(WIZARD_STEP_KEY);
    if (stored && steps.includes(stored as Step)) return stored as Step;
  } catch { /* localStorage unavailable */ }
  return 'welcome';
}

export function Onboarding({ onComplete }: OnboardingProps) {
  const { t } = useTranslation();

  const stepLabels: Record<Step, string> = {
    welcome: t('onboarding.stepLabel.welcome'),
    taste: t('onboarding.stepLabel.taste', 'Taste Test'),
    setup: t('onboarding.stepLabel.setup'),
    calibrate: t('onboarding.stepLabel.calibrate', 'Calibrate'),
  };
  const [step, setStep] = useState<Step>(getPersistedStep);
  const [isAnimating, setIsAnimating] = useState(true);

  const currentIndex = steps.indexOf(step);

  // Persist wizard step to localStorage
  useEffect(() => {
    try { localStorage.setItem(WIZARD_STEP_KEY, step); } catch { /* noop */ }
  }, [step]);

  // Trigger entrance animation on step change
  useEffect(() => {
    setIsAnimating(true);
    const timer = setTimeout(() => setIsAnimating(false), 300);
    return () => clearTimeout(timer);
  }, [step]);

  const nextStep = () => {
    const next = steps[currentIndex + 1];
    if (next) setStep(next);
  };

  const prevStep = () => {
    const prev = steps[currentIndex - 1];
    if (prev) setStep(prev);
  };

  const handleSetupComplete = async () => {
    try {
      await invoke('mark_onboarding_complete');
    } catch {
      // Non-critical — continue anyway
    }
    try { localStorage.removeItem(WIZARD_STEP_KEY); } catch { /* noop */ }
    onComplete();
  };

  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-bg-primary p-8" role="dialog" aria-modal="true" aria-label="Setup wizard">
      {/* Progress indicator */}
      <div className="absolute top-8 flex flex-col items-center gap-2">
        {/* Step X of Y text */}
        <p className="text-xs text-text-muted">
          {t('onboarding.stepProgress', { current: currentIndex + 1, total: steps.length, defaultValue: 'Step {{current}} of {{total}}' })}
        </p>

        {/* Step circles */}
        <div className="flex items-center gap-1" role="group" aria-label={`Step ${currentIndex + 1} of ${steps.length}: ${stepLabels[step]}`}>
          {steps.map((s, i) => (
            <div key={s} className="flex items-center">
              <div
                className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-medium transition-all duration-300 ${
                  i < currentIndex
                    ? 'bg-orange-500 text-white'
                    : i === currentIndex
                    ? 'bg-orange-500/20 text-orange-400 ring-2 ring-orange-500'
                    : 'bg-bg-tertiary text-text-muted'
                }`}
              >
                {i < currentIndex ? '\u2713' : i + 1}
              </div>
              {i < steps.length - 1 && (
                <div
                  className={`w-8 h-0.5 transition-colors duration-300 ${
                    i < currentIndex ? 'bg-orange-500' : 'bg-bg-tertiary'
                  }`}
                />
              )}
            </div>
          ))}
        </div>

        {/* Progress bar */}
        <div className="w-48 h-1 bg-bg-tertiary rounded-full overflow-hidden">
          <div
            className="h-full bg-orange-500 rounded-full transition-all duration-500 ease-out"
            style={{ width: `${((currentIndex + 1) / steps.length) * 100}%` }}
          />
        </div>

        {/* Step label */}
        <div className="text-xs text-text-muted">
          {stepLabels[step]}
        </div>
      </div>

      {/* Step content */}
      <div className="max-w-2xl w-full">
        {step === 'welcome' && (
          <WelcomeStep isAnimating={isAnimating} onNext={nextStep} />
        )}

        {step === 'taste' && (
          <TasteTestStep
            isAnimating={isAnimating}
            onComplete={nextStep}
            onSkip={nextStep}
          />
        )}

        {step === 'setup' && (
          <QuickSetupStep
            isAnimating={isAnimating}
            onComplete={nextStep}
            onBack={prevStep}
          />
        )}

        {step === 'calibrate' && (
          <CalibrationStep
            isAnimating={isAnimating}
            onComplete={handleSetupComplete}
            onBack={prevStep}
          />
        )}
      </div>

      {/* Version */}
      <p className="absolute bottom-6 text-xs text-text-muted">{t('onboarding.version', { version: __APP_VERSION__ })}</p>
    </div>
  );
}
