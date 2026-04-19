// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { GuidedHighlights } from '../GuidedHighlights';

describe('GuidedHighlights', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders 5 highlight dots when not previously shown', () => {
    render(<GuidedHighlights />);
    const dots = screen.getAllByRole('button', { name: /highlight/i });
    expect(dots).toHaveLength(5);
  });

  it('does not render when localStorage flag is already set', () => {
    localStorage.setItem('4da_highlights_shown', 'true');
    const { container } = render(<GuidedHighlights />);
    expect(container.innerHTML).toBe('');
  });

  it('dismisses a dot on click and shows tooltip text', () => {
    render(<GuidedHighlights />);
    const dots = screen.getAllByRole('button', { name: /highlight/i });
    fireEvent.click(dots[0]!);
    expect(screen.getByText('guided.feed')).toBeInTheDocument();
  });

  it('auto-dismisses all highlights after 30 seconds', () => {
    render(<GuidedHighlights />);
    expect(screen.getAllByRole('button', { name: /highlight/i })).toHaveLength(5);
    act(() => { vi.advanceTimersByTime(30000); });
    expect(localStorage.getItem('4da_highlights_shown')).toBe('true');
  });

  it('sets localStorage flag when all highlights are dismissed manually', () => {
    render(<GuidedHighlights />);
    const dots = screen.getAllByRole('button', { name: /highlight/i });
    dots.forEach(dot => fireEvent.click(dot));
    // After tooltips fade, flag should be set
    act(() => { vi.advanceTimersByTime(3500); });
    expect(localStorage.getItem('4da_highlights_shown')).toBe('true');
  });
});
