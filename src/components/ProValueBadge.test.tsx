// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

// Configurable mock state
let mockState: Record<string, unknown> = {};
function setMockState(overrides: Record<string, unknown>) {
  mockState = {
    proValueReport: null,
    ...overrides,
  };
}

vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector(mockState)),
}));

import { ProValueBadge } from './ProValueBadge';

describe('ProValueBadge', () => {
  it('returns null when no report is available', () => {
    setMockState({ proValueReport: null });
    const { container } = render(<ProValueBadge />);
    expect(container.innerHTML).toBe('');
  });

  it('returns null when all metrics are zero', () => {
    setMockState({
      proValueReport: {
        signals_detected: 0,
        knowledge_gaps_caught: 0,
        estimated_hours_saved: 0,
        period_days: 30,
      },
    });
    const { container } = render(<ProValueBadge />);
    expect(container.innerHTML).toBe('');
  });

  it('returns null when only signals_detected > 0 (vanity metric removed)', () => {
    setMockState({
      proValueReport: {
        signals_detected: 5,
        knowledge_gaps_caught: 0,
        estimated_hours_saved: 0,
        period_days: 30,
      },
    });
    const { container } = render(<ProValueBadge />);
    expect(container.innerHTML).toBe('');
  });

  it('renders gaps count when knowledge_gaps_caught > 0', () => {
    setMockState({
      proValueReport: {
        signals_detected: 0,
        knowledge_gaps_caught: 3,
        estimated_hours_saved: 0,
        period_days: 30,
      },
    });
    render(<ProValueBadge />);
    expect(screen.getByText(/pro\.gaps/)).toBeInTheDocument();
  });

  it('renders hours saved when estimated_hours_saved > 0', () => {
    setMockState({
      proValueReport: {
        signals_detected: 0,
        knowledge_gaps_caught: 0,
        estimated_hours_saved: 2.5,
        period_days: 30,
      },
    });
    render(<ProValueBadge />);
    expect(screen.getByText(/pro\.hoursSaved/)).toBeInTheDocument();
  });

  it('renders all metrics separated by dots when all are present', () => {
    setMockState({
      proValueReport: {
        signals_detected: 5,
        knowledge_gaps_caught: 3,
        estimated_hours_saved: 2.5,
        period_days: 30,
      },
    });
    render(<ProValueBadge />);
    // The component joins parts with ' · '
    const badge = screen.getByText(/·/);
    expect(badge).toBeInTheDocument();
  });

  it('has a title attribute with intelligence summary', () => {
    setMockState({
      proValueReport: {
        signals_detected: 5,
        knowledge_gaps_caught: 3,
        estimated_hours_saved: 1.5,
        period_days: 30,
      },
    });
    render(<ProValueBadge />);
    const badge = screen.getByTitle(/pro\.intelligenceSummary/);
    expect(badge).toBeInTheDocument();
  });
});
