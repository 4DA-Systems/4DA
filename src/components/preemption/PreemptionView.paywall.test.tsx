// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import PreemptionView from './PreemptionView';

// AB-011 render guard: the slice classifies the Signal gate into a paywalled
// flag (covered by preemption-slice.test.ts); THIS verifies the view actually
// renders the upgrade CTA for that flag — and NOT the red error banner. The
// slice test proves the wiring; this proves the JSX.

vi.mock('../../hooks/use-cold-start-gate', () => ({ useColdStartGate: () => false }));
vi.mock('../SignalUpgradeCTA', () => ({
  SignalUpgradeCTA: () => <div data-testid="signal-upgrade-cta" />,
}));

let mockState: Record<string, unknown> = {};
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

function setState(overrides: Record<string, unknown>) {
  mockState = {
    preemptionFeed: null,
    preemptionLoading: false,
    preemptionError: null,
    preemptionPaywalled: false,
    loadPreemption: vi.fn(),
    ...overrides,
  };
}

describe('PreemptionView — paywall render', () => {
  it('renders the upgrade CTA + lock copy when paywalled, NOT an error banner', () => {
    setState({ preemptionPaywalled: true });
    render(<PreemptionView />);

    // localized lock copy (i18n test setup returns the key)
    expect(screen.getByText('preemption.locked.title')).toBeInTheDocument();
    expect(screen.getByText('preemption.locked.subtitle')).toBeInTheDocument();
    // the actual upgrade CTA is rendered
    expect(screen.getByTestId('signal-upgrade-cta')).toBeInTheDocument();
  });

  it('does NOT render a red error banner in the paywalled state', () => {
    setState({ preemptionPaywalled: true });
    const { container } = render(<PreemptionView />);
    // the error branch uses border-red-500/30 — must be absent
    expect(container.querySelector('.border-red-500\\/30')).toBeNull();
  });

  it('renders a genuine error (not the CTA) when error is set', () => {
    setState({ preemptionError: 'database is locked' });
    render(<PreemptionView />);
    expect(screen.getByText('database is locked')).toBeInTheDocument();
    expect(screen.queryByTestId('signal-upgrade-cta')).toBeNull();
  });
});
