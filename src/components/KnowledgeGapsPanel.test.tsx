// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Fresh-profile E2E 2026-10-09: a user who skipped the project scan was told
// "No gaps detected — your knowledge is current". With no dependency known,
// nothing was checked; the panel now offers the scan instead.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act, fireEvent } from '@testing-library/react';

const cmdMock = vi.fn();
vi.mock('../lib/commands', () => ({
  cmd: (...args: unknown[]) => cmdMock(...args),
}));

// "Your knowledge is current" rests on the news having been read: a source
// check succeeded, the data is not stale, an engine cycle linked it. A
// stack-only profile (AD-054) with a handful of fetched items qualifies.
const NEWS_READ = {
  newest_item_age_hours: 1, items_last_24h: 6, items_last_72h: 6,
  newest_source_check_age_hours: 0.5, source_checks_last_24h: 6, source_checks_last_72h: 6,
  failing_sources: 0, stale_sources: 0, total_sources: 6, is_stale: false, no_recent_fetches: false,
  last_run_items_scored: 6, last_run_sources_succeeded: 6, last_run_sources_failed: 0,
  last_run_sources_skipped: 0,
  last_run_age_minutes: 20, computed_at: null,
};

vi.mock('./ProGate', () => ({
  ProGate: ({ children }: { children: React.ReactNode }) => <div data-testid="pro-gate">{children}</div>,
}));

vi.mock('./ContentTranslationProvider', () => ({
  useTranslatedContent: () => ({ getTranslated: (_id: string, text: string) => text }),
}));

const runAutoDiscovery = vi.fn(() => Promise.resolve());
const loadUserContext = vi.fn(() => Promise.resolve());
const startAnalysis = vi.fn(() => Promise.resolve());
vi.mock('../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ isScanning: false, runAutoDiscovery, loadUserContext, startAnalysis }),
}));

import { KnowledgeGapsPanel } from './KnowledgeGapsPanel';

describe('KnowledgeGapsPanel', () => {
  beforeEach(() => {
    cmdMock.mockReset();
    runAutoDiscovery.mockClear();
  });

  it('never claims "current" when no dependency is known, and offers the scan', async () => {
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 0 });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});

    expect(screen.getByTestId('knowledge-gaps-scan-prompt')).toHaveTextContent('knowledgeGaps.noDependencies');
    expect(screen.queryByText(/your knowledge is current/)).not.toBeInTheDocument();
    expect(screen.queryByText('knowledgeGaps.noGaps')).not.toBeInTheDocument();

    await act(async () => {
      fireEvent.click(screen.getByText('onboarding.choice.scanProjects'));
    });
    expect(runAutoDiscovery).toHaveBeenCalledTimes(1);
  });

  it('offers the scan on day one too: no lockfile is not a cold-start silence', async () => {
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 0, data_freshness: null });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByTestId('knowledge-gaps-scan-prompt')).toBeInTheDocument();
  });

  // Wave 9e: after a restart the previous run's gaps serve while this run's
  // compute. The panel says how old they are, then swaps the fresh ones in.
  it('labels the previous run\'s gaps and quietly swaps in the recomputed ones', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: false });
    try {
      const hoursAgo = new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString();
      cmdMock.mockResolvedValueOnce({ items: [], total: 0, total_tracked: 94, computed_at: hoursAgo, data_freshness: NEWS_READ });
      cmdMock.mockResolvedValueOnce({ items: [], total: 0, total_tracked: 94, computed_at: new Date().toISOString(), data_freshness: NEWS_READ });
      render(<KnowledgeGapsPanel />);
      await act(async () => {});
      expect(screen.getByTestId('knowledge-gaps-report-age').textContent).toBe('knowledgeGaps.reportAge');

      await act(async () => { vi.advanceTimersByTime(15_000); });
      expect(cmdMock).toHaveBeenCalledTimes(2);
      expect(screen.queryByTestId('knowledge-gaps-report-age')).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it('says nothing about gaps computed this cycle', async () => {
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 94, computed_at: new Date().toISOString(), data_freshness: NEWS_READ });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.queryByTestId('knowledge-gaps-report-age')).not.toBeInTheDocument();
  });

  it('keeps the all-clear for a user whose dependencies were checked', async () => {
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 94, data_freshness: NEWS_READ });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.queryByTestId('knowledge-gaps-scan-prompt')).not.toBeInTheDocument();
    expect(screen.getByText('knowledgeGaps.noGaps')).toBeInTheDocument();
  });

  // AD-054: a Preemption sub-view of its own, so every state must say
  // something true — never a blank panel, never a false all-clear.
  it('renders the gaps expanded, one card per gap', async () => {
    cmdMock.mockResolvedValue({ items: [gapItem('serde', 'high'), gapItem('tokio', 'medium')], total_tracked: 94 });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByText('serde')).toBeInTheDocument();
    expect(screen.getByText('tokio')).toBeInTheDocument();
    expect(screen.getByText('knowledgeGaps.needAttention')).toBeInTheDocument();
  });

  it('a Signal gate renders the upgrade path, not a blank panel or an all-clear', async () => {
    cmdMock.mockRejectedValue('Knowledge Gaps requires 4DA Signal — start your free trial or upgrade to unlock it.');
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByTestId('pro-gate')).toBeInTheDocument();
    expect(screen.getByTestId('knowledge-gaps-gated')).toBeInTheDocument();
    expect(screen.queryByText('knowledgeGaps.noGaps')).not.toBeInTheDocument();
  });

  it('any other failure offers Retry and never claims "current"', async () => {
    cmdMock.mockRejectedValueOnce(new Error('database is locked'));
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByTestId('knowledge-gaps-error')).toBeInTheDocument();
    expect(screen.queryByText('knowledgeGaps.noGaps')).not.toBeInTheDocument();

    cmdMock.mockResolvedValueOnce({ items: [], total_tracked: 94, data_freshness: NEWS_READ });
    await act(async () => {
      fireEvent.click(screen.getByText('action.retry'));
    });
    expect(screen.getByText('knowledgeGaps.noGaps')).toBeInTheDocument();
  });

  it('says nothing beyond its description before any source was read', async () => {
    cmdMock.mockResolvedValue({ items: [], total_tracked: 94, data_freshness: null });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByText('knowledgeGaps.subtitle')).toBeInTheDocument();
    expect(screen.queryByText('knowledgeGaps.noGaps')).not.toBeInTheDocument();
  });

  it('says nothing when the news it read is stale', async () => {
    cmdMock.mockResolvedValue({ items: [], total_tracked: 94, data_freshness: { ...NEWS_READ, is_stale: true } });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.queryByText('knowledgeGaps.noGaps')).not.toBeInTheDocument();
  });

  it('says nothing until an engine cycle has linked what was fetched', async () => {
    cmdMock.mockResolvedValue({ items: [], total_tracked: 94, data_freshness: { ...NEWS_READ, last_run_age_minutes: null } });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.queryByText('knowledgeGaps.noGaps')).not.toBeInTheDocument();
  });

  it('a small stack-only profile may be told it is current: no item-count floor', async () => {
    // Six stack items, far below the retired 100-item threshold.
    cmdMock.mockResolvedValue({ items: [], total_tracked: 3, data_freshness: NEWS_READ });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByText('knowledgeGaps.noGaps')).toBeInTheDocument();
  });

  it('real gaps render even before the news is confirmed fresh', async () => {
    cmdMock.mockResolvedValue({ items: [gapItem('serde', 'high')], total_tracked: 94, data_freshness: null });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByText('serde')).toBeInTheDocument();
  });
});

function gapItem(dep: string, urgency: string) {
  return {
    id: `kg_${dep}`, kind: 'gap', title: `Knowledge gap: ${dep}`, explanation: 'unread release notes',
    confidence: { value: 0.6, provenance: 'heuristic', sample_size: null },
    urgency, reversibility: null,
    evidence: [{ source: 'hn', title: `${dep} 2.0 released`, url: 'https://example.com', freshness_days: 2, relevance_note: null }],
    affected_projects: ['/code/app'], affected_deps: [dep], suggested_actions: [],
    precedents: [], refutation_condition: null,
    lens_hints: { briefing: false, preemption: false, blind_spots: false, evidence: false, other_build_target: false, upgrade_plan: false, no_coverage: false },
    created_at: 0, expires_at: null,
  };
}
