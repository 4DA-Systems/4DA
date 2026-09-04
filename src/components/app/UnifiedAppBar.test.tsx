// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Mocks — the bar pulls in the brand mark, void signals, command search and
// several status dots; none of them are under test here.
// ---------------------------------------------------------------------------
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string | Record<string, unknown>) =>
      typeof fallback === 'string' ? fallback : key,
  }),
}));
vi.mock('../void-engine/BrandMark', () => ({ BrandMark: () => <div data-testid="brand-mark" /> }));
vi.mock('../../hooks/use-void-signals', () => ({ useVoidSignals: () => 'idle' }));
vi.mock('../OllamaStatus', () => ({ OllamaStatus: () => null }));
vi.mock('../SystemHealthDot', () => ({ SystemHealthDot: () => null }));
vi.mock('../ThemeToggle', () => ({ ThemeToggle: () => null }));
vi.mock('../search/CommandSearch', () => ({ CommandSearch: () => null }));
vi.mock('../../lib/commands', () => ({ cmd: vi.fn(() => Promise.resolve()) }));
vi.mock('../../store', () => ({
  useAppStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ appState: { progress: 0 }, generateBriefing: vi.fn() }),
}));

import { UnifiedAppBar } from './UnifiedAppBar';

const UNJUDGED_TOOLTIP = 'Fast pass, LLM judge not applied yet';

function renderBar(overrides: Partial<Parameters<typeof UnifiedAppBar>[0]> = {}) {
  return render(
    <UnifiedAppBar
      state={{ loading: false, analysisComplete: true }}
      monitoring={null}
      settingsFormProvider="anthropic"
      isPro={false}
      tier="FREE"
      summaryBadges={{ relevantCount: 69, topCount: 12, total: 580 }}
      judged={false}
      aiBriefing={{ error: null }}
      onAnalyze={() => {}}
      onOpenSettings={() => {}}
      analysisPulse={false}
      {...overrides}
    />,
  );
}

describe('UnifiedAppBar — unjudged badge on the relevant chip', () => {
  it('badges an unjudged run next to the relevant count, with the fast-pass tooltip', () => {
    // The fresh-launch run is `foreground_fast` (llm_rerank: false): its "69
    // relevant" is pipeline scores only. Without the badge that reads as a
    // judged 69 — the exact confusion measured live on 2026-09-04.
    renderBar({ judged: false });
    const badge = screen.getByTestId('unjudged-badge');
    expect(badge).toHaveTextContent('unjudged');
    expect(badge).toHaveAttribute('title', UNJUDGED_TOOLTIP);
    expect(badge).toHaveAttribute('aria-label', UNJUDGED_TOOLTIP);
    // …and the count itself is still there, unchanged.
    expect(screen.getByText('69')).toBeInTheDocument();
  });

  it('drops the badge once the backend reports a judged result set', () => {
    renderBar({ judged: true });
    expect(screen.queryByTestId('unjudged-badge')).toBeNull();
    expect(screen.getByText('69')).toBeInTheDocument();
  });

  it('shows no badge while there is no chip to badge (analysis not complete)', () => {
    renderBar({ judged: false, state: { loading: true, analysisComplete: false } });
    expect(screen.queryByTestId('unjudged-badge')).toBeNull();
    expect(screen.queryByText('69')).toBeNull();
  });

  it('shows no badge without summary badges even when unjudged', () => {
    renderBar({ judged: false, summaryBadges: null });
    expect(screen.queryByTestId('unjudged-badge')).toBeNull();
  });
});
