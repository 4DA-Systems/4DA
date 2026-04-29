// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { URGENCY_CONFIG, ItemCard } from './PreemptionCard';
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

const makeItem = (overrides: Partial<EvidenceItem> = {}): EvidenceItem => ({
  id: 'preempt-1',
  kind: 'alert',
  title: 'React 19 breaking change',
  explanation: 'This affects your project dependencies',
  urgency: 'high',
  confidence: { value: 0.85, provenance: 'llm_assessed', sample_size: 10 },
  reversibility: null,
  evidence: [{
    url: 'https://react.dev/blog',
    source: 'github',
    title: 'React 19 release',
    freshness_days: 2,
    relevance_note: 'Direct dependency update',
  }],
  affected_deps: ['react'],
  affected_projects: ['my-app'],
  suggested_actions: [],
  precedents: [],
  refutation_condition: null,
  lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false },
  created_at: BigInt(Date.now()),
  expires_at: null,
  ...overrides,
});

describe('URGENCY_CONFIG', () => {
  it('defines all four urgency levels', () => {
    expect(Object.keys(URGENCY_CONFIG)).toEqual(['critical', 'high', 'medium', 'watch']);
  });

  it('uses i18n labelKey references', () => {
    for (const [, cfg] of Object.entries(URGENCY_CONFIG)) {
      expect(cfg.labelKey).toMatch(/^preemption\.urgency\./);
    }
  });
});

describe('ItemCard', () => {
  const surfacedRef = { current: new Set<string>() } as React.RefObject<Set<string>>;

  it('renders item title', () => {
    render(<ItemCard item={makeItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.getByText('React 19 breaking change')).toBeDefined();
  });

  it('renders confidence percentage', () => {
    render(<ItemCard item={makeItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.getByText('85%')).toBeDefined();
  });

  it('renders explanation text', () => {
    render(<ItemCard item={makeItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.getByText('This affects your project dependencies')).toBeDefined();
  });

  it('renders affected deps as chips', () => {
    render(<ItemCard item={makeItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.getByText('react')).toBeDefined();
  });

  it('renders evidence source', () => {
    render(<ItemCard item={makeItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.getByText('github')).toBeDefined();
  });
});
