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
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    expect(screen.getByText('Start using 4DA')).toBeInTheDocument();
    expect(screen.getByText('Continue setup')).toBeInTheDocument();
  });

  it('calls onStartUsing when primary button is clicked', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    fireEvent.click(screen.getByText('Start using 4DA'));
    expect(mockStartUsing).toHaveBeenCalledTimes(1);
    expect(mockContinueSetup).not.toHaveBeenCalled();
  });

  it('calls onContinueSetup when secondary button is clicked', () => {
    render(
      <OnboardingChoiceGate
        isAnimating={false}
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
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
      />,
    );

    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).toContain('opacity-0');
    expect(wrapper.className).toContain('scale-95');
  });
});
