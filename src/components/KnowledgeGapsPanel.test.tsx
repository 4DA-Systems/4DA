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

let coldStart = false;
vi.mock('../hooks/use-cold-start-gate', () => ({
  useColdStartGate: () => coldStart,
}));

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
    coldStart = false;
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
    coldStart = true;
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 0 });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByTestId('knowledge-gaps-scan-prompt')).toBeInTheDocument();
  });

  it('keeps the all-clear for a user whose dependencies were checked', async () => {
    cmdMock.mockResolvedValue({ items: [], total: 0, total_tracked: 94 });
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

    cmdMock.mockResolvedValueOnce({ items: [], total_tracked: 94 });
    await act(async () => {
      fireEvent.click(screen.getByText('action.retry'));
    });
    expect(screen.getByText('knowledgeGaps.noGaps')).toBeInTheDocument();
  });

  it('says nothing beyond its description on day one with no gaps', async () => {
    coldStart = true;
    cmdMock.mockResolvedValue({ items: [], total_tracked: 94 });
    render(<KnowledgeGapsPanel />);
    await act(async () => {});
    expect(screen.getByText('knowledgeGaps.subtitle')).toBeInTheDocument();
    expect(screen.queryByText('knowledgeGaps.noGaps')).not.toBeInTheDocument();
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
