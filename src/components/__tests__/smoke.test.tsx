/**
 * Batch smoke tests for display/presentational components.
 *
 * Verifies that each component can render without throwing.
 * Catches import errors, missing providers, and render crashes.
 */
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Tauri API mocks — must appear before component imports
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Store mock — provides default slice values for all components that use Zustand
// ---------------------------------------------------------------------------
vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
      const mockState: Record<string, unknown> = {
        // app state
        activeView: 'briefing',
        setActiveView: vi.fn(),
        embeddingMode: null,
        setShowSettings: vi.fn(),
        appState: {
          loading: false,
          analysisComplete: false,
          status: 'Ready',
          relevanceResults: [],
          progress: 0,
          progressStage: '',
        },
        userContext: null,
        startAnalysis: vi.fn(),
        generateBriefing: vi.fn(),
        aiBriefing: {
          content: null,
          loading: false,
          error: null,
          model: null,
        },

        // playbook slice
        playbookModules: [],
        playbookContent: null,
        playbookProgress: null,
        playbookLoading: false,
        playbookError: null,
        activeModuleId: null,
        streetsTier: 'playbook',
        loadPlaybookModules: vi.fn(),
        loadPlaybookContent: vi.fn(),
        loadPlaybookProgress: vi.fn(),
        markLessonComplete: vi.fn(),
        loadStreetsTier: vi.fn(),
        activateStreetsLicense: vi.fn(),
        personalizedLessons: {},
        loadPersonalizedContent: vi.fn(),
        loadPersonalizedContentBatch: vi.fn(),

        // decisions
        decisions: [],
        decisionsLoading: false,
        loadDecisions: vi.fn(),
        recordDecision: vi.fn(),
        updateDecision: vi.fn(),

        // compound advantage
        compoundAdvantage: null,
        loadCompoundAdvantage: vi.fn(),

        // autophagy
        autophagyStatus: null,
        autophagyHistory: [],
        autophagyLoading: false,
        loadAutophagyStatus: vi.fn(),
        loadAutophagyHistory: vi.fn(),

        // decision windows
        decisionWindows: [],
        decisionWindowsLoading: false,
        loadDecisionWindows: vi.fn(),
        actOnWindow: vi.fn(),
        dismissWindow: vi.fn(),

        // pro value
        proValueReport: null,

        // agent memory
        agentMemories: [],
        agentMemoryLoading: false,
        loadAgentMemories: vi.fn(),

        // progressive disclosure
        viewTier: 'core',
        showAllViews: false,
        analysisCycleCount: 0,
        firstAnalysisDate: null,
        setShowAllViews: vi.fn(),
        incrementAnalysisCycle: vi.fn(),
        computeViewTier: vi.fn(),

        // license
        tier: 'free',
        trialStatus: null,
        expired: false,
        daysRemaining: 0,
        expiresAt: null,
        startTrial: vi.fn(),
        activateLicense: vi.fn(),
      };
      return selector(mockState);
    }),
    {
      // Static getState/setState used by some components (e.g., ActionBar, PlaybookView)
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

// Mock zustand/react/shallow
vi.mock('zustand/react/shallow', () => ({
  useShallow: (fn: unknown) => fn,
}));

// Mock use-license hook (used by ProGate)
vi.mock('../../hooks/use-license', () => ({
  useLicense: () => ({
    tier: 'free',
    isPro: false,
    trialStatus: null,
    expired: false,
    daysRemaining: 0,
    expiresAt: null,
  }),
}));

// Mock sun-logo image import
vi.mock('../../assets/sun-logo.webp', () => ({
  default: 'mock-sun-logo.webp',
}));

// Mock playbook markdown renderer
vi.mock('../../utils/playbook-markdown', () => ({
  renderMarkdown: () => 'rendered markdown',
}));

// Mock playbook sub-components used by PlaybookView
vi.mock('../playbook/SovereignProfile', () => ({
  SovereignProfile: () => null,
}));
vi.mock('../playbook/StreetHealthBadge', () => ({
  StreetHealthBadge: () => null,
}));
vi.mock('../playbook/SovereignInsightCard', () => ({
  SovereignInsightCard: () => null,
}));
vi.mock('../playbook/SovereignConnectionBlock', () => ({
  SovereignConnectionBlock: () => null,
}));
vi.mock('../playbook/DiffRibbon', () => ({
  DiffRibbon: () => null,
}));
vi.mock('../playbook/FeedEchoBlock', () => ({
  FeedEchoBlock: () => null,
}));
vi.mock('../playbook/ProgressiveRevealBanner', () => ({
  ProgressiveRevealBanner: () => null,
}));
vi.mock('../playbook/PersonalizationDepthIndicator', () => ({
  PersonalizationDepthIndicator: () => null,
}));
vi.mock('../playbook/TemplateLibrary', () => ({
  TemplateLibrary: () => null,
}));

// Mock error messages util
vi.mock('../../utils/error-messages', () => ({
  translateError: (e: unknown) => String(e),
}));

// Mock first-run messages util
vi.mock('../../utils/first-run-messages', () => ({
  getStageNarration: () => 'narration',
  getSourceNarration: () => 'source narration',
  getCelebrationMessage: () => 'celebration',
}));

// Mock score utils
vi.mock('../../utils/score', () => ({
  formatScore: (s: number) => `${Math.round(s * 100)}%`,
  getScoreColor: () => 'text-white',
  getStageLabel: (s: string) => s || 'Ready',
}));

// Mock source config
vi.mock('../../config/sources', () => ({
  getSourceLabel: (s: string) => s,
  getSourceColorClass: () => 'bg-gray-500/20 text-gray-400',
  getSourceFullName: (s: string) => s,
}));

// Mock content types config
vi.mock('../../config/content-types', () => ({
  getContentTypeBadge: () => null,
}));

// Mock game-components and game-icons (used by AchievementsPanel, ActionBar, etc.)
vi.mock('../../lib/game-components', () => ({
  registerGameComponent: vi.fn(() => Promise.resolve()),
}));

vi.mock('../../lib/game-icons', () => ({
  getGameIcon: () => null,
}));

// ---------------------------------------------------------------------------
// Component imports
// ---------------------------------------------------------------------------
import { ConfidenceIndicator } from '../ConfidenceIndicator';
import { ToastContainer } from '../Toast';
import { KeyboardShortcutsModal } from '../KeyboardShortcutsModal';
import { BriefingCard } from '../BriefingCard';
import { ErrorBoundary } from '../ErrorBoundary';
import { LearningIndicator } from '../LearningIndicator';
import { BadgeRow } from '../result-item/BadgeRow';
import { ScoreBreakdownRow } from '../result-item/ScoreBreakdownRow';
import { AboutPanel } from '../AboutPanel';
import { ProGate } from '../ProGate';
import { ProValueBadge } from '../ProValueBadge';

import { ViewTabBar } from '../ViewTabBar';
import { PlaybookView } from '../PlaybookView';
import { DecisionMemory } from '../DecisionMemory';
import { FeedbackButtons } from '../result-item/FeedbackButtons';
import { ProInsightRow } from '../result-item/ProInsightRow';
import { StreetsEngineLink } from '../result-item/StreetsEngineLink';
import { BriefingLoadingState, BriefingReadyState, BriefingNoDataState } from '../BriefingEmptyStates';
import { SignalsPanel } from '../SignalsPanel';
import { OllamaStatus } from '../OllamaStatus';
import { CompoundAdvantageScore } from '../CompoundAdvantageScore';
import { AutophagyInsights } from '../AutophagyInsights';
import { LearnedBehaviorPanel } from '../LearnedBehaviorPanel';
import { SplashScreen } from '../SplashScreen';
import { VoidHeartbeat } from '../void-engine/VoidHeartbeat';
import { ActionBar } from '../ActionBar';

// ---------------------------------------------------------------------------
// Shared test data
// ---------------------------------------------------------------------------
import { makeItem, makeBreakdown, makeSettings } from '../../test/factories';

// ---------------------------------------------------------------------------
// Smoke test suite
// ---------------------------------------------------------------------------
const SMOKE_COMPONENTS: Array<{
  name: string;
  render: () => React.JSX.Element;
}> = [
  // --- Original 18 components ---
  {
    name: 'ConfidenceIndicator (with confidence)',
    render: () => <ConfidenceIndicator confidence={0.85} />,
  },
  {
    name: 'ConfidenceIndicator (no confidence)',
    render: () => <ConfidenceIndicator />,
  },
  {
    name: 'ToastContainer (empty)',
    render: () => <ToastContainer toasts={[]} onDismiss={vi.fn()} />,
  },
  {
    name: 'ToastContainer (with toasts)',
    render: () => (
      <ToastContainer
        toasts={[
          { id: 1, message: 'Test toast', type: 'success' as const },
          { id: 2, message: 'Error toast', type: 'error' as const },
        ]}
        onDismiss={vi.fn()}
      />
    ),
  },
  {
    name: 'KeyboardShortcutsModal',
    render: () => <KeyboardShortcutsModal onClose={vi.fn()} />,
  },
  {
    name: 'BriefingCard',
    render: () => (
      <BriefingCard
        item={makeItem()}
        onSave={vi.fn()}
        onDismiss={vi.fn()}
      />
    ),
  },
  {
    name: 'ErrorBoundary (no error)',
    render: () => (
      <ErrorBoundary>
        <div>Child content</div>
      </ErrorBoundary>
    ),
  },
  {
    name: 'LearningIndicator (empty)',
    render: () => (
      <LearningIndicator learnedAffinities={[]} antiTopics={[]} />
    ),
  },
  {
    name: 'LearningIndicator (with data)',
    render: () => (
      <LearningIndicator
        learnedAffinities={[
          { topic: 'rust', positive_signals: 5, negative_signals: 1, affinity_score: 0.8 },
          { topic: 'react', positive_signals: 3, negative_signals: 0, affinity_score: 0.6 },
        ]}
        antiTopics={[
          { topic: 'cobol', rejection_count: 4, confidence: 0.9, auto_detected: true },
        ]}
      />
    ),
  },
  {
    name: 'BadgeRow',
    render: () => <BadgeRow item={makeItem()} />,
  },
  {
    name: 'ScoreBreakdownRow',
    render: () => (
      <ScoreBreakdownRow
        item={makeItem({ score_breakdown: makeBreakdown() })}
        isTopPick={true}
        isHighConfidence={true}
      />
    ),
  },
  {
    name: 'AboutPanel',
    render: () => <AboutPanel />,
  },
  {
    name: 'ProGate (free tier)',
    render: () => (
      <ProGate feature="test feature">
        <div>Pro content</div>
      </ProGate>
    ),
  },
  {
    name: 'ProValueBadge',
    render: () => <ProValueBadge />,
  },
  {
    name: 'ViewTabBar',
    render: () => <ViewTabBar />,
  },
  {
    name: 'PlaybookView (no module selected)',
    render: () => <PlaybookView />,
  },
  {
    name: 'DecisionMemory (empty)',
    render: () => <DecisionMemory />,
  },

  // --- New components (19-38) ---
  {
    name: 'FeedbackButtons (no feedback)',
    render: () => (
      <FeedbackButtons
        item={makeItem()}
        feedback={undefined}
        onRecordInteraction={vi.fn()}
      />
    ),
  },
  {
    name: 'FeedbackButtons (saved)',
    render: () => (
      <FeedbackButtons
        item={makeItem()}
        feedback="save"
        onRecordInteraction={vi.fn()}
      />
    ),
  },
  {
    name: 'FeedbackButtons (dismissed)',
    render: () => (
      <FeedbackButtons
        item={makeItem()}
        feedback="dismiss"
        onRecordInteraction={vi.fn()}
      />
    ),
  },
  {
    name: 'ProInsightRow (no signal)',
    render: () => <ProInsightRow item={makeItem({ top_score: 0.2 })} />,
  },
  {
    name: 'ProInsightRow (with signal)',
    render: () => (
      <ProInsightRow
        item={makeItem({
          top_score: 0.7,
          signal_type: 'security_alert',
          score_breakdown: makeBreakdown({ context_score: 0.5 }),
        })}
      />
    ),
  },
  {
    name: 'StreetsEngineLink (no engine)',
    render: () => <StreetsEngineLink item={makeItem()} />,
  },
  {
    name: 'StreetsEngineLink (with engine)',
    render: () => (
      <StreetsEngineLink
        item={makeItem({
          streets_engine: 'Engine 1: Digital Products',
          title: 'Show HN: Build AI agents with Rust',
        })}
      />
    ),
  },
  {
    name: 'BriefingLoadingState',
    render: () => <BriefingLoadingState />,
  },
  {
    name: 'BriefingReadyState',
    render: () => <BriefingReadyState />,
  },
  {
    name: 'BriefingNoDataState',
    render: () => <BriefingNoDataState />,
  },
  {
    name: 'SignalsPanel (empty results)',
    render: () => <SignalsPanel results={[]} />,
  },
  {
    name: 'SignalsPanel (with signals)',
    render: () => (
      <SignalsPanel
        results={[
          makeItem({
            id: 1,
            signal_type: 'security_alert',
            signal_priority: 'critical',
            signal_action: 'Update immediately',
            signal_triggers: ['CVE-2025-1234'],
          }),
          makeItem({
            id: 2,
            signal_type: 'tech_trend',
            signal_priority: 'medium',
            signal_action: 'Monitor this trend',
            signal_triggers: ['trending'],
          }),
        ]}
      />
    ),
  },
  {
    name: 'OllamaStatus (non-ollama provider)',
    render: () => <OllamaStatus provider="openai" />,
  },
  {
    name: 'OllamaStatus (ollama provider)',
    render: () => <OllamaStatus provider="ollama" />,
  },
  {
    name: 'CompoundAdvantageScore (null advantage)',
    render: () => <CompoundAdvantageScore />,
  },
  {
    name: 'AutophagyInsights',
    render: () => <AutophagyInsights />,
  },
  {
    name: 'LearnedBehaviorPanel (empty)',
    render: () => (
      <LearnedBehaviorPanel
        affinities={[]}
        antiTopics={[]}
        onRefresh={vi.fn()}
      />
    ),
  },
  {
    name: 'LearnedBehaviorPanel (with data)',
    render: () => (
      <LearnedBehaviorPanel
        affinities={[
          { topic: 'tauri', positive_signals: 8, negative_signals: 0, affinity_score: 0.95 },
        ]}
        antiTopics={[
          { topic: 'php', rejection_count: 3, confidence: 0.85, auto_detected: true },
        ]}
        onRefresh={vi.fn()}
      />
    ),
  },
  {
    name: 'SplashScreen',
    render: () => <SplashScreen onComplete={vi.fn()} minimumDisplayTime={0} />,
  },
  {
    name: 'VoidHeartbeat (default signal)',
    render: () => (
      <VoidHeartbeat
        signal={{
          pulse: 0, heat: 0, burst: 0, morph: 0, error: 0,
          staleness: 0, item_count: 0, signal_intensity: 0,
          signal_urgency: 0, critical_count: 0, signal_color_shift: 0,
          metabolism: 0, open_windows: 0, advantage_trend: 0,
        }}
      />
    ),
  },
  {
    name: 'ActionBar (idle state)',
    render: () => (
      <ActionBar
        state={{
          loading: false,
          analysisComplete: false,
          status: 'Ready',
          lastAnalyzedAt: null,
          progress: 0,
          progressStage: '',
          relevanceResults: [],
        }}
        settings={makeSettings()}
        aiBriefing={{ loading: false, error: null }}
        autoBriefingEnabled={false}
        summaryBadges={null}
        onAnalyze={vi.fn()}
        onGenerateBriefing={vi.fn()}
        onToggleAutoBriefing={vi.fn()}
        onToast={vi.fn()}
      />
    ),
  },
];

describe('Component smoke tests', () => {
  SMOKE_COMPONENTS.forEach(({ name, render: renderComponent }) => {
    it(`${name} renders without throwing`, () => {
      expect(() => {
        const { unmount } = render(renderComponent());
        unmount();
      }).not.toThrow();
    });
  });
});
