import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type { Step } from './onboarding/types';
import { WelcomeStep } from './onboarding/WelcomeStep';
import { QuickSetupStep } from './onboarding/QuickSetupStep';

interface OnboardingProps {
  onComplete: () => void;
}

const steps: Step[] = ['welcome', 'setup'];

const stepLabels: Record<Step, string> = {
  welcome: 'Welcome',
  setup: 'Setup',
};

export function Onboarding({ onComplete }: OnboardingProps) {
  const [step, setStep] = useState<Step>('welcome');
  const [isAnimating, setIsAnimating] = useState(true);

  const currentIndex = steps.indexOf(step);

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
    onComplete();
  };

  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-[#0A0A0A] p-8">
      {/* Progress indicator */}
      <div className="absolute top-8 flex items-center gap-1">
        {steps.map((s, i) => (
          <div key={s} className="flex items-center">
            <div
              className={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-medium transition-all duration-300 ${
                i < currentIndex
                  ? 'bg-orange-500 text-white'
                  : i === currentIndex
                  ? 'bg-orange-500/20 text-orange-400 ring-2 ring-orange-500'
                  : 'bg-[#1F1F1F] text-gray-600'
              }`}
            >
              {i < currentIndex ? '\u2713' : i + 1}
            </div>
            {i < steps.length - 1 && (
              <div
                className={`w-8 h-0.5 transition-colors duration-300 ${
                  i < currentIndex ? 'bg-orange-500' : 'bg-[#1F1F1F]'
                }`}
              />
            )}
          </div>
        ))}
      </div>

      {/* Step label */}
      <div className="absolute top-[70px] text-xs text-gray-500">
        {stepLabels[step]}
      </div>

      {/* Step content */}
      <div className="max-w-2xl w-full">
        {step === 'welcome' && (
          <WelcomeStep isAnimating={isAnimating} onNext={nextStep} />
        )}

        {step === 'setup' && (
          <QuickSetupStep
            isAnimating={isAnimating}
            onComplete={handleSetupComplete}
            onBack={prevStep}
          />
        )}
      </div>

      {/* Version */}
      <p className="absolute bottom-6 text-xs text-gray-600">Version 1.0.0</p>
    </div>
  );
}
