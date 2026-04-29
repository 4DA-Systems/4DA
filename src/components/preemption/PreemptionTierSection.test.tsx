// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { PreemptionTierSection } from './PreemptionTierSection';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(() => Promise.resolve(null)),
}));

vi.mock('../../lib/trust-feedback', () => ({
  recordTrustEvent: vi.fn(),
}));

const makeItem = (id: string, title: string): EvidenceItem => ({
  id,
  kind: 'alert',
  title,
  explanation: 'test explanation',
  urgency: 'high',
  confidence: { value: 0.8, provenance: 'heuristic', sample_size: null },
  reversibility: null,
  evidence: [{
    url: 'https://example.com',
    source: 'test',
    title: 'evidence',
    freshness_days: 1,
    relevance_note: 'test',
  }],
  affected_deps: [],
  affected_projects: [],
  suggested_actions: [],
  precedents: [],
  refutation_condition: null,
  lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false },
  created_at: BigInt(Date.now()),
  expires_at: null,
});

describe('PreemptionTierSection', () => {
  const defaultProps = {
    dotColor: '#EF4444',
    borderColor: 'rgba(239, 68, 68, 0.2)',
    title: 'Your Stack',
    subtitle: '2 affect your dependencies',
    items: [] as EvidenceItem[],
    surfacedRef: { current: new Set<string>() } as React.RefObject<Set<string>>,
    onDismiss: vi.fn(),
    emptyText: 'No alerts right now',
  };

  it('renders empty state when no items', () => {
    render(<PreemptionTierSection {...defaultProps} />);
    expect(screen.getByText('No alerts right now')).toBeDefined();
  });

  it('renders section title and subtitle', () => {
    render(<PreemptionTierSection {...defaultProps} />);
    expect(screen.getByText('Your Stack')).toBeDefined();
    expect(screen.getByText('2 affect your dependencies')).toBeDefined();
  });

  it('renders items when provided', () => {
    const items = [makeItem('item-1', 'Critical Update'), makeItem('item-2', 'Breaking Change')];
    render(<PreemptionTierSection {...defaultProps} items={items} />);
    expect(screen.getByText('Critical Update')).toBeDefined();
    expect(screen.getByText('Breaking Change')).toBeDefined();
  });

  it('has correct aria-label', () => {
    render(<PreemptionTierSection {...defaultProps} />);
    expect(screen.getByRole('region', { name: 'Your Stack' })).toBeDefined();
  });
});
