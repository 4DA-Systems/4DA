import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';

import type { Step } from './onboarding/types';
import { WelcomeStep } from './onboarding/WelcomeStep';
import { TasteTestStep } from './onboarding/TasteTestStep';
import { OnboardingChoiceGate } from './onboarding/OnboardingChoiceGate';
import { QuickSetupStep } from './onboarding/QuickSetupStep';
import { CalibrationStep } from './onboarding/CalibrationStep';

interface OnboardingProps {
  onComplete: () => void;
}

// All logical steps including the choice gate
const steps: Step[] = ['welcome', 'taste', 'choice', 'setup', 'calibrate'];

// Steps shown in the progress indicator (choice is a decision point, not a visible step)
const displaySteps: Step[] = ['welcome', 'taste', 'setup', 'calibrate'];

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
    choice: t('onboarding.stepLabel.taste', 'Taste Test'),
    setup: t('onboarding.stepLabel.setup'),
    calibrate: t('onboarding.stepLabel.calibrate', 'Calibrate'),
  };
  const [step, setStep] = useState<Step>(getPersistedStep);
  const [isAnimating, setIsAnimating] = useState(true);
  const modalRef = useRef<HTMLDivElement>(null);

  const currentIndex = steps.indexOf(step);

  // Map from actual step to display index for the progress indicator.
  // The choice step visually sits between taste (1) and setup (2), showing
  // taste as complete. We treat it as display index 2 (same as setup) so
  // the progress bar lands at 50%.
  const displayIndex = step === 'choice'
    ? displaySteps.indexOf('setup')
    : displaySteps.indexOf(step);

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

  // Focus trap: keep Tab cycling within the onboarding wizard
  useEffect(() => {
    const modal = modalRef.current;
    if (!modal) return;

    // Auto-focus first focusable element on step change
    const focusable = modal.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
    );
    focusable[0]?.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      const els = modal.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
      );
      const first = els[0];
      const last = els[els.length - 1];
      if (e.shiftKey) {
        if (document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };

    modal.addEventListener('keydown', handleKeyDown);
    return () => modal.removeEventListener('keydown', handleKeyDown);
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
      await cmd('mark_onboarding_complete');
    } catch {
      // Non-critical — continue anyway
    }
    try { localStorage.removeItem(WIZARD_STEP_KEY); } catch { /* noop */ }
    onComplete();
  };

  const handleSkipToContent = async () => {
    try {
      await cmd('mark_onboarding_complete');
    } catch {
      // Non-critical — continue anyway
    }
    try { localStorage.removeItem(WIZARD_STEP_KEY); } catch { /* noop */ }
    onComplete();
  };

  // Hide progress indicator on the choice gate — it's a decision point, not a step
  const showProgress = step !== 'choice';

  return (
    <div ref={modalRef} className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-bg-primary p-8" role="dialog" aria-modal="true" aria-label="Setup wizard">
      {/* Progress indicator — hidden during choice gate */}
      {showProgress && (
        <div className="absolute top-8 flex flex-col items-center gap-2">
          {/* Step X of Y text */}
          <p className="text-xs text-text-muted">
            {t('onboarding.stepProgress', { current: displayIndex + 1, total: displaySteps.length, defaultValue: 'Step {{current}} of {{total}}' })}
          </p>

          {/* Step circles — only display steps, not the choice gate */}
          <div className="flex items-center gap-1" role="group" aria-label={`Step ${displayIndex + 1} of ${displaySteps.length}: ${stepLabels[step]}`}>
            {displaySteps.map((s, i) => (
              <div key={s} className="flex items-center">
                <div
                  className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-medium transition-all duration-300 ${
                    i < displayIndex
                      ? 'bg-orange-500 text-white'
                      : i === displayIndex
                      ? 'bg-orange-500/20 text-orange-400 ring-2 ring-orange-500'
                      : 'bg-bg-tertiary text-text-muted'
                  }`}
                >
                  {i < displayIndex ? '\u2713' : i + 1}
                </div>
                {i < displaySteps.length - 1 && (
                  <div
                    className={`w-8 h-0.5 transition-colors duration-300 ${
                      i < displayIndex ? 'bg-orange-500' : 'bg-bg-tertiary'
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
              style={{ width: `${((displayIndex + 1) / displaySteps.length) * 100}%` }}
            />
          </div>

          {/* Step label */}
          <div className="text-xs text-text-muted">
            {stepLabels[step]}
          </div>
        </div>
      )}

      {/* Step content */}
      <div className="max-w-2xl w-full">
        {step === 'welcome' && (
          <WelcomeStep isAnimating={isAnimating} onNext={nextStep} onSkip={handleSkipToContent} />
        )}

        {step === 'taste' && (
          <TasteTestStep
            isAnimating={isAnimating}
            onComplete={nextStep}
            onSkip={nextStep}
          />
        )}

        {step === 'choice' && (
          <OnboardingChoiceGate
            isAnimating={isAnimating}
            onStartUsing={handleSkipToContent}
            onContinueSetup={nextStep}
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
