// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

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
  const mockScanProjects = vi.fn();

  beforeEach(() => {
    mockStartUsing.mockClear();
    mockContinueSetup.mockClear();
    mockScanProjects.mockClear();
  });

  const renderGate = (hasProviderConfigured = false, isAnimating = false) =>
    render(
      <OnboardingChoiceGate
        isAnimating={isAnimating}
        hasProviderConfigured={hasProviderConfigured}
        onStartUsing={mockStartUsing}
        onContinueSetup={mockContinueSetup}
        onScanProjects={mockScanProjects}
      />,
    );

  it('renders all three choice paths', () => {
    renderGate();

    expect(screen.getByText('Scan my projects')).toBeInTheDocument();
    expect(screen.getByText('Continue full setup')).toBeInTheDocument();
    expect(screen.getByText(/keyword matching only/i)).toBeInTheDocument();
  });

  it('calls onScanProjects when the scan button is clicked', () => {
    mockScanProjects.mockResolvedValue(undefined);
    renderGate();

    fireEvent.click(screen.getByText('Scan my projects'));
    expect(mockScanProjects).toHaveBeenCalledTimes(1);
    expect(mockContinueSetup).not.toHaveBeenCalled();
    expect(mockStartUsing).not.toHaveBeenCalled();
  });

  it('shows an inline scanning state while the scan runs', async () => {
    // Never-resolving promise keeps the scanning state visible.
    mockScanProjects.mockReturnValue(new Promise(() => {}));
    renderGate();

    fireEvent.click(screen.getByText('Scan my projects'));
    await waitFor(() => {
      expect(screen.getByText(/Scanning your projects/i)).toBeInTheDocument();
    });
    // The choice buttons are replaced by the scanning state.
    expect(screen.queryByText('Continue full setup')).not.toBeInTheDocument();
  });

  it('calls onContinueSetup when the continue button is clicked', () => {
    renderGate();

    fireEvent.click(screen.getByText('Continue full setup'));
    expect(mockContinueSetup).toHaveBeenCalledTimes(1);
    expect(mockScanProjects).not.toHaveBeenCalled();
  });

  it('calls onStartUsing when the keyword-only button is clicked', () => {
    renderGate();

    fireEvent.click(screen.getByText(/keyword matching only/i));
    expect(mockStartUsing).toHaveBeenCalledTimes(1);
    expect(mockScanProjects).not.toHaveBeenCalled();
  });

  it('shows the keyword-only hint text', () => {
    renderGate();

    expect(screen.getByText(/scan or add a provider anytime/i)).toBeInTheDocument();
  });

  it('applies animation classes when isAnimating is true', () => {
    const { container } = renderGate(false, true);

    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper.className).toContain('opacity-0');
    expect(wrapper.className).toContain('scale-95');
  });

  it('hides the provider status chip when no provider is configured', () => {
    renderGate(false);

    // A bare "AI Provider" chip on the path-choice screen read as a stray heading;
    // it is suppressed until a provider is actually configured.
    expect(screen.queryByText('AI Provider configured')).not.toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('makes "Scan my projects" the primary recommended action', () => {
    renderGate();

    const scanBtn = screen.getByText('Scan my projects').closest('button')!;
    expect(scanBtn.className).toContain('bg-orange-500');
    expect(scanBtn.className).toContain('text-lg');
    expect(screen.getByText('Recommended')).toBeInTheDocument();
  });

  it('shows green status for configured provider', () => {
    renderGate(true);

    const status = screen.getByRole('status');
    expect(status.className).toContain('text-green-400');
    expect(screen.getByText('AI Provider configured')).toBeInTheDocument();
  });
});
