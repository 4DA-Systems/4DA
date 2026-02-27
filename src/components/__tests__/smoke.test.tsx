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
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
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

      // street health
      streetHealth: null,
      loadStreetHealth: vi.fn(),

      // suns dashboard
      sunsModules: [],
      sunsAlerts: [],
      sunsLoading: false,
      loadSunsModules: vi.fn(),
      ackSunAlert: vi.fn(),
      executeSunRecommendation: vi.fn(),

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
      autophagyLoading: false,
      loadAutophagyStatus: vi.fn(),

      // pro value
      proValueReport: null,

      // game
      celebration: null,
      clearCelebration: vi.fn(),

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
vi.mock('../../assets/sun-logo.jpg', () => ({
  default: 'mock-sun-logo.jpg',
}));

// Mock playbook markdown renderer
vi.mock('../../utils/playbook-markdown', () => ({
  renderMarkdown: () => 'rendered markdown',
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
import { GameCelebration } from '../GameCelebration';
import { ViewTabBar } from '../ViewTabBar';
import { PlaybookView } from '../PlaybookView';
import { DecisionMemory } from '../DecisionMemory';

// ---------------------------------------------------------------------------
// Shared test data
// ---------------------------------------------------------------------------
import { makeItem, makeBreakdown } from '../../test/factories';

// ---------------------------------------------------------------------------
// Smoke test suite
// ---------------------------------------------------------------------------
const SMOKE_COMPONENTS: Array<{
  name: string;
  render: () => React.JSX.Element;
}> = [
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
    name: 'GameCelebration (no celebration)',
    render: () => <GameCelebration />,
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
