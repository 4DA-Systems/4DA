// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.reject('not mocked')),
}));

import { RadarSVG } from '../RadarSVG';
import type { RadarEntry } from '../RadarSVG';

const mockEntries: RadarEntry[] = [
  {
    name: 'Rust',
    ring: 'adopt',
    quadrant: 'languages',
    movement: 'stable',
    signals: ['Rust 1.80 released'],
    decision_ref: null,
    score: 0.95,
  },
  {
    name: 'Tauri',
    ring: 'trial',
    quadrant: 'frameworks',
    movement: 'up',
    signals: ['Tauri 2.0 stable', 'New plugin ecosystem'],
    decision_ref: 1,
    score: 0.88,
  },
];

describe('RadarSVG', () => {
  const onEntryClick = vi.fn();

  it('renders SVG with role="img" and aria-label', () => {
    render(<RadarSVG entries={mockEntries} userStack={[]} onEntryClick={onEntryClick} />);
    const svg = screen.getByRole('img');
    expect(svg).toBeInTheDocument();
    expect(svg).toHaveAttribute('aria-label', 'techRadar.svgLabel');
  });

  it('renders ring labels with i18n keys', () => {
    render(<RadarSVG entries={mockEntries} userStack={[]} onEntryClick={onEntryClick} />);
    expect(screen.getByText('techRadar.ringAdopt')).toBeInTheDocument();
    expect(screen.getByText('techRadar.ringTrial')).toBeInTheDocument();
    expect(screen.getByText('techRadar.ringAssess')).toBeInTheDocument();
    expect(screen.getByText('techRadar.ringHold')).toBeInTheDocument();
  });

  it('renders entry dots with role="button" and aria-label', () => {
    render(<RadarSVG entries={mockEntries} userStack={[]} onEntryClick={onEntryClick} />);
    const buttons = screen.getAllByRole('button');
    // Entry dots + potential back button (not zoomed, so just entry dots)
    expect(buttons.length).toBeGreaterThanOrEqual(2);

    // Check entry dot aria-labels contain entry name and metadata
    const rustDot = screen.getByLabelText(/Rust/);
    expect(rustDot).toBeInTheDocument();
    const tauriDot = screen.getByLabelText(/Tauri/);
    expect(tauriDot).toBeInTheDocument();
  });

  it('entry dots are keyboard accessible with tabIndex', () => {
    render(<RadarSVG entries={mockEntries} userStack={[]} onEntryClick={onEntryClick} />);
    const dots = screen.getAllByRole('button');
    for (const dot of dots) {
      expect(dot).toHaveAttribute('tabindex', '0');
    }
  });

  it('renders quadrant labels', () => {
    render(<RadarSVG entries={mockEntries} userStack={[]} onEntryClick={onEntryClick} />);
    expect(screen.getByText('Languages')).toBeInTheDocument();
    expect(screen.getByText('Frameworks')).toBeInTheDocument();
    expect(screen.getByText('Tools')).toBeInTheDocument();
    expect(screen.getByText('Platforms')).toBeInTheDocument();
  });

  it('highlights user stack entries with gold ring', () => {
    const { container } = render(
      <RadarSVG entries={mockEntries} userStack={['Rust']} onEntryClick={onEntryClick} />,
    );
    // Gold ring (stroke="#D4AF37") should be present for Rust
    const goldCircles = container.querySelectorAll('circle[stroke="#D4AF37"]');
    expect(goldCircles.length).toBeGreaterThanOrEqual(1);
  });

  it('renders with empty entries without crashing', () => {
    const { container } = render(
      <RadarSVG entries={[]} userStack={[]} onEntryClick={onEntryClick} />,
    );
    expect(container.querySelector('svg')).toBeInTheDocument();
  });
});
