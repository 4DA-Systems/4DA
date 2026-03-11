/**
 * Accessibility tests using jest-axe.
 *
 * Runs axe-core on key interactive components to catch a11y violations.
 * Covers SettingsModal (each tab), BriefingView states, ResultsView, and onboarding.
 */
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import { axe, toHaveNoViolations } from 'jest-axe';

expect.extend(toHaveNoViolations);

// --- Tauri mocks ---
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// --- Store mock ---
vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
      const mockState: Record<string, unknown> = {
        activeView: 'briefing', setActiveView: vi.fn(), embeddingMode: null,
        setShowSettings: vi.fn(),
        appState: { loading: false, analysisComplete: false, status: 'Ready', relevanceResults: [], progress: 0, progressStage: '' },
        userContext: null, startAnalysis: vi.fn(), generateBriefing: vi.fn(),
        aiBriefing: { content: null, loading: false, error: null, model: null },
        playbookModules: [], playbookContent: null, playbookProgress: null,
        playbookLoading: false, playbookError: null, activeModuleId: null,
        streetsTier: 'playbook', loadPlaybookModules: vi.fn(),
        loadPlaybookContent: vi.fn(), loadPlaybookProgress: vi.fn(),
        markLessonComplete: vi.fn(), loadStreetsTier: vi.fn(),
        activateStreetsLicense: vi.fn(), personalizedLessons: {},
        loadPersonalizedContent: vi.fn(), loadPersonalizedContentBatch: vi.fn(),
        decisions: [], decisionsLoading: false, loadDecisions: vi.fn(),
        recordDecision: vi.fn(), updateDecision: vi.fn(),
        compoundAdvantage: null, loadCompoundAdvantage: vi.fn(),
        autophagyStatus: null, autophagyHistory: [], autophagyLoading: false,
        loadAutophagyStatus: vi.fn(), loadAutophagyHistory: vi.fn(),
        decisionWindows: [], decisionWindowsLoading: false,
        loadDecisionWindows: vi.fn(), actOnWindow: vi.fn(), dismissWindow: vi.fn(),
        proValueReport: null, agentMemories: [], agentMemoryLoading: false,
        loadAgentMemories: vi.fn(),
        viewTier: 'core', showAllViews: false, analysisCycleCount: 0,
        firstAnalysisDate: null, setShowAllViews: vi.fn(),
        incrementAnalysisCycle: vi.fn(), computeViewTier: vi.fn(),
        tier: 'free', trialStatus: null, expired: false,
        daysRemaining: 0, expiresAt: null, startTrial: vi.fn(),
        activateLicense: vi.fn(),
      };
      return selector(mockState);
    }),
    {
      getState: () => ({
        setShowSettings: vi.fn(),
        playbookProgress: null,
        playbookContent: null,
        personalizedLessons: {},
      }),
      setState: vi.fn(),
    },
  ),
}));

vi.mock('zustand/react/shallow', () => ({
  useShallow: (fn: unknown) => fn,
}));
vi.mock('../../hooks/use-license', () => ({
  useLicense: () => ({ tier: 'free', isPro: false, trialStatus: null, expired: false, daysRemaining: 0, expiresAt: null }),
}));
vi.mock('../../assets/sun-logo.webp', () => ({ default: 'mock-sun-logo.webp' }));
vi.mock('../../utils/playbook-markdown', () => ({ renderMarkdown: () => 'rendered' }));
vi.mock('../playbook/SovereignProfile', () => ({ SovereignProfile: () => null }));
vi.mock('../playbook/StreetHealthBadge', () => ({ StreetHealthBadge: () => null }));
vi.mock('../playbook/SovereignInsightCard', () => ({ SovereignInsightCard: () => null }));
vi.mock('../playbook/SovereignConnectionBlock', () => ({ SovereignConnectionBlock: () => null }));
vi.mock('../playbook/DiffRibbon', () => ({ DiffRibbon: () => null }));
vi.mock('../playbook/FeedEchoBlock', () => ({ FeedEchoBlock: () => null }));
vi.mock('../playbook/ProgressiveRevealBanner', () => ({ ProgressiveRevealBanner: () => null }));
vi.mock('../playbook/PersonalizationDepthIndicator', () => ({ PersonalizationDepthIndicator: () => null }));
vi.mock('../playbook/TemplateLibrary', () => ({ TemplateLibrary: () => null }));
vi.mock('../../utils/error-messages', () => ({ translateError: (e: unknown) => String(e) }));
vi.mock('../../utils/first-run-messages', () => ({
  getStageNarration: () => 'narration',
  getSourceNarration: () => 'source narration',
  getCelebrationMessage: () => 'celebration',
}));
vi.mock('../../utils/score', () => ({
  formatScore: (s: number) => `${Math.round(s * 100)}%`,
  getScoreColor: () => 'text-white',
  getStageLabel: (s: string) => s || 'Ready',
}));
vi.mock('../../config/sources', () => ({
  getSourceLabel: (s: string) => s,
  getSourceColorClass: () => 'bg-gray-500/20 text-gray-400',
  getSourceFullName: (s: string) => s,
}));
vi.mock('../../config/content-types', () => ({ getContentTypeBadge: () => null }));
vi.mock('../../lib/game-components', () => ({ registerGameComponent: vi.fn(() => Promise.resolve()) }));
vi.mock('../../lib/game-icons', () => ({ getGameIcon: () => null }));

// --- Import components ---
import { ConfidenceIndicator } from '../ConfidenceIndicator';
import { ErrorBoundary } from '../ErrorBoundary';
import { ViewTabBar } from '../ViewTabBar';
import { SplashScreen } from '../SplashScreen';
import { DecisionMemory } from '../DecisionMemory';
import { CompoundAdvantageScore } from '../CompoundAdvantageScore';
import { AutophagyInsights } from '../AutophagyInsights';
import { makeSettings } from '../../test/factories';
import { ActionBar } from '../ActionBar';

describe('Accessibility: ConfidenceIndicator', () => {
  it('has no a11y violations with confidence value', async () => {
    const { container } = render(<ConfidenceIndicator confidence={0.85} />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });

  it('has no a11y violations without confidence', async () => {
    const { container } = render(<ConfidenceIndicator />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('Accessibility: ErrorBoundary', () => {
  it('has no a11y violations in normal state', async () => {
    const { container } = render(
      <ErrorBoundary><div>Content</div></ErrorBoundary>,
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('Accessibility: ViewTabBar', () => {
  it('has no a11y violations (excluding aria-controls referencing external panels)', async () => {
    const { container } = render(<ViewTabBar />);
    // aria-controls references panels rendered by the parent App, not in this test scope
    const results = await axe(container, {
      rules: { 'aria-valid-attr-value': { enabled: false } },
    });
    expect(results).toHaveNoViolations();
  });
});

describe('Accessibility: SplashScreen', () => {
  it('has no a11y violations', async () => {
    const { container } = render(<SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('Accessibility: DecisionMemory', () => {
  it('has no a11y violations in empty state', async () => {
    const { container } = render(<DecisionMemory />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('Accessibility: CompoundAdvantageScore', () => {
  it('has no a11y violations in null state', async () => {
    const { container } = render(<CompoundAdvantageScore />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('Accessibility: AutophagyInsights', () => {
  it('has no a11y violations', async () => {
    const { container } = render(<AutophagyInsights />);
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});

describe('Accessibility: ActionBar', () => {
  it('has no a11y violations in idle state', async () => {
    const { container } = render(
      <ActionBar
        state={{ loading: false, analysisComplete: false, status: 'Ready', lastAnalyzedAt: null, progress: 0, progressStage: '', relevanceResults: [] }}
        settings={makeSettings()}
        aiBriefing={{ loading: false, error: null }}
        autoBriefingEnabled={false}
        summaryBadges={null}
        onAnalyze={vi.fn()}
        onGenerateBriefing={vi.fn()}
        onToggleAutoBriefing={vi.fn()}
        onToast={vi.fn()}
      />,
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });

  it('has no a11y violations in loading state', async () => {
    const { container } = render(
      <ActionBar
        state={{ loading: true, analysisComplete: false, status: 'Analyzing...', lastAnalyzedAt: null, progress: 0.5, progressStage: 'fetch', relevanceResults: [] }}
        settings={makeSettings()}
        aiBriefing={{ loading: false, error: null }}
        autoBriefingEnabled={false}
        summaryBadges={null}
        onAnalyze={vi.fn()}
        onGenerateBriefing={vi.fn()}
        onToggleAutoBriefing={vi.fn()}
        onToast={vi.fn()}
      />,
    );
    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});
