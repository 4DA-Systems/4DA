// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// i18n mock — return key as text (with defaultValue fallback)
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => (typeof fallback === 'string' ? fallback : key),
  }),
}));

import { OnboardingChoiceGate } from './OnboardingChoiceGate';

describe('OnboardingChoiceGate', () => {
  const mockStartUsing = vi.fn();
  const mockContinueSetup = vi.fn();

  beforeEach(() => {
    mockStartUsing.mockClear();
    mockContinueSetup.mockClear();
  });

  it('renders both choice buttons', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={false}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    expect(screen.getByText('Start using 4DA')).toBeInTheDocument();
    expect(screen.getByText('Continue setup')).toBeInTheDocument();
  });

  it('calls onStartUsing when start button is clicked', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={true}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    fireEvent.click(screen.getByText('Start using 4DA'));
    expect(mockStartUsing).toHaveBeenCalledTimes(1);
    expect(mockContinueSetup).not.toHaveBeenCalled();
  });

  it('calls onContinueSetup when continue button is clicked', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={false}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    fireEvent.click(screen.getByText('Continue setup'));
    expect(mockContinueSetup).toHaveBeenCalledTimes(1);
    expect(mockStartUsing).not.toHaveBeenCalled();
  });

  it('shows background analysis hint text', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={false}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    expect(screen.getByText('Analysis continues in the background')).toBeInTheDocument();
  });

  it('applies animation classes when isAnimating is true', () => {
    const { container } = render(
      <OnboardingChoiceGate
        isAnimating={true}
        hasProviderConfigured={false}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).toContain('opacity-0');
    expect(wrapper.className).toContain('scale-95');
  });

  it('shows AI Provider status indicator', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={false}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    expect(screen.getByText('AI Provider')).toBeInTheDocument();
  });

  it('shows recommendation hint when provider is not configured', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={false}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    expect(
      screen.getByText('Recommended: configure an AI provider for the best experience'),
    ).toBeInTheDocument();
  });

  it('makes "Continue setup" primary when provider is not configured', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={false}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    const continueBtn = screen.getByText('Continue setup').closest('button')!;
    expect(continueBtn.className).toContain('bg-orange-500');
    expect(continueBtn.className).toContain('text-lg');
  });

  it('makes "Start using 4DA" primary when provider is configured', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={true}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    const startBtn = screen.getByText('Start using 4DA').closest('button')!;
    expect(startBtn.className).toContain('bg-orange-500');
    expect(startBtn.className).toContain('text-lg');
  });

  it('shows green status for configured provider', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        hasProviderConfigured={true}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    const status = screen.getByRole('status');
    expect(status.className).toContain('text-green-400');
  });
});
