// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

const mockSetActiveView = vi.fn();
vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
    const mockState: Record<string, unknown> = {
      setActiveView: mockSetActiveView,
    };
    return selector(mockState);
  }),
}));

import { FirstCveCard } from '../FirstCveCard';

const defaultProps = {
  cveId: 'CVE-2026-1234',
  packageName: 'lodash',
  severity: 'high',
  projectCount: 3,
  minutesSincePublication: 12,
};

describe('FirstCveCard', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  it('renders CVE details when not previously shown', () => {
    render(<FirstCveCard {...defaultProps} />);
    expect(screen.getByText('4DA Protected You')).toBeInTheDocument();
    expect(screen.getByText(/CVE-2026-1234/)).toBeInTheDocument();
    expect(screen.getByText(/lodash/)).toBeInTheDocument();
  });

  it('does not render when localStorage flag is set', () => {
    localStorage.setItem('4da_first_cve_shown', 'true');
    const { container } = render(<FirstCveCard {...defaultProps} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows detection time', () => {
    render(<FirstCveCard {...defaultProps} />);
    expect(screen.getByText(/12 minutes/)).toBeInTheDocument();
  });

  it('dismisses and sets localStorage on Got it click', () => {
    render(<FirstCveCard {...defaultProps} />);
    fireEvent.click(screen.getByText('Got it'));
    expect(localStorage.getItem('4da_first_cve_shown')).toBe('true');
  });

  it('navigates to toolkit view on View Details click', () => {
    render(<FirstCveCard {...defaultProps} />);
    fireEvent.click(screen.getByText('View Details'));
    expect(mockSetActiveView).toHaveBeenCalledWith('toolkit');
  });
});
