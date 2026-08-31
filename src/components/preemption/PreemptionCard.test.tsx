// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { URGENCY_CONFIG, ItemCard } from './PreemptionCard';
import { cmd } from '../../lib/commands';
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

const mockCmd = vi.mocked(cmd);

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
  lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: false, no_coverage: false },
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

  it('renders VERIFIED badge for osv_verified provenance', () => {
    const item = makeItem({ confidence: { value: 0.95, provenance: 'osv_verified', sample_size: null } });
    render(<ItemCard item={item} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.getByText('preemption.badge.verified')).toBeDefined();
  });

  it('renders AI badge for llm_assessed provenance', () => {
    const item = makeItem({ confidence: { value: 0.7, provenance: 'llm_assessed', sample_size: 5 } });
    render(<ItemCard item={item} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.getByText('preemption.badge.ai')).toBeDefined();
  });

  it('does not render tier badge for heuristic provenance', () => {
    const item = makeItem({ confidence: { value: 0.5, provenance: 'heuristic', sample_size: null } });
    render(<ItemCard item={item} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.queryByText('preemption.badge.verified')).toBeNull();
    expect(screen.queryByText('preemption.badge.ai')).toBeNull();
  });

  it('renders the "other build target" badge when lens_hints.other_build_target is set (Phase 2c)', () => {
    const item = makeItem({
      lens_hints: { briefing: false, preemption: true, blind_spots: false, evidence: false, other_build_target: true, upgrade_plan: false, no_coverage: false },
    });
    render(<ItemCard item={item} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.getByText('preemption.otherTargets.badge')).toBeDefined();
  });

  it('does NOT render the other-build-target badge for a normal item', () => {
    render(<ItemCard item={makeItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    expect(screen.queryByText('preemption.otherTargets.badge')).toBeNull();
  });
});

// ─── Lazy detail hydration (AD-035) ─────────────────────────────────────────
// The LIST response embeds only what the collapsed card renders (evidence
// capped, `evidence_total` recording the real count, explanation byte-capped).
// The first expansion fetches the full item from get_preemption_item_detail;
// a fetch failure still expands the embedded rows.

describe('ItemCard — lazy expansion of a list-trimmed item', () => {
  const surfacedRef = { current: new Set<string>() } as React.RefObject<Set<string>>;

  const embeddedEvidence = [
    { url: 'https://osv.dev/1', source: 'osv', title: 'embedded row one', freshness_days: 2, relevance_note: '' },
    { url: 'https://osv.dev/2', source: 'osv', title: 'embedded row two', freshness_days: 3, relevance_note: '' },
  ];
  const hiddenEvidence = [
    { url: 'https://osv.dev/3', source: 'osv', title: 'hydrated row three', freshness_days: 4, relevance_note: 'CVSS 9.1' },
    { url: 'https://osv.dev/4', source: 'osv', title: 'hydrated row four', freshness_days: 5, relevance_note: 'CVSS 7.5' },
  ];

  const trimmedItem = (): EvidenceItem =>
    makeItem({
      evidence: embeddedEvidence,
      evidence_total: embeddedEvidence.length + hiddenEvidence.length,
    });

  const fullItem = (): EvidenceItem =>
    makeItem({
      evidence: [...embeddedEvidence, ...hiddenEvidence],
      evidence_total: null,
    });

  beforeEach(() => {
    mockCmd.mockReset();
    mockCmd.mockImplementation(() => Promise.resolve(null as never));
  });

  it('counts the held-back citations in the header and the show-more control', () => {
    const { container } = render(
      <ItemCard item={trimmedItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />,
    );
    // Header: 2 embedded + 2 held back = (4); the expand control renders even
    // though only 2 rows are embedded.
    expect(container.querySelector('h4')?.textContent).toBe('preemption.evidence (4)');
    expect(screen.getByText('preemption.evidence.showMore')).toBeInTheDocument();
  });

  it('fetches the full item on "show more" and renders the hydrated rows', async () => {
    mockCmd.mockResolvedValue(fullItem() as never);
    render(<ItemCard item={trimmedItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);

    expect(screen.queryByText('hydrated row three')).toBeNull();
    fireEvent.click(screen.getByText('preemption.evidence.showMore'));

    await waitFor(() => {
      expect(screen.getByText('hydrated row three')).toBeInTheDocument();
      expect(screen.getByText('hydrated row four')).toBeInTheDocument();
    });
    expect(mockCmd).toHaveBeenCalledWith('get_preemption_item_detail', { itemId: 'preempt-1' });
  });

  it('still expands the embedded rows when the detail fetch fails', async () => {
    mockCmd.mockRejectedValue(new Error('backend gone'));
    render(<ItemCard item={trimmedItem()} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);

    fireEvent.click(screen.getByText('preemption.evidence.showMore'));
    await waitFor(() => {
      // Both embedded rows visible; collapse control present; no crash.
      expect(screen.getByText('embedded row one')).toBeInTheDocument();
      expect(screen.getByText('embedded row two')).toBeInTheDocument();
      expect(screen.getByText('preemption.evidence.showLess')).toBeInTheDocument();
    });
  });

  it('never fetches for a complete (non-trimmed) item', () => {
    const complete = makeItem({
      evidence: [...embeddedEvidence, ...hiddenEvidence],
      evidence_total: null,
    });
    render(<ItemCard item={complete} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);
    fireEvent.click(screen.getByText('preemption.evidence.showMore'));
    expect(mockCmd).not.toHaveBeenCalledWith('get_preemption_item_detail', expect.anything());
  });

  it('hydrates the full explanation when a transport-capped explanation expands', async () => {
    const longFull = 'B'.repeat(500);
    mockCmd.mockResolvedValue(makeItem({ explanation: longFull, evidence_total: null }) as never);
    const capped = makeItem({
      explanation: `${'B'.repeat(320)}…`,
      evidence: embeddedEvidence,
      evidence_total: 2,
    });
    render(<ItemCard item={capped} surfacedRef={surfacedRef} onDismiss={vi.fn()} />);

    fireEvent.click(screen.getByText('preemption.explanation.expand'));
    await waitFor(() => {
      expect(mockCmd).toHaveBeenCalledWith('get_preemption_item_detail', { itemId: 'preempt-1' });
      expect(screen.getByText(new RegExp(`^${'B'.repeat(500)}`))).toBeInTheDocument();
    });
  });
});
