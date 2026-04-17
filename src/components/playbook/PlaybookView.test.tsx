import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Tauri API mocks
// ---------------------------------------------------------------------------
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Store mock — uses globalThis so the hoisted vi.mock can access mutable state
// ---------------------------------------------------------------------------
const mockLoadModules = vi.fn();
const mockLoadContent = vi.fn();
const mockLoadProgress = vi.fn();
const mockMarkComplete = vi.fn();
const mockLoadStreetHealth = vi.fn();


function makeDefaultStore(): Record<string, unknown> {
  return {
    playbookModules: [],
    playbookContent: null,
    playbookProgress: null,
    playbookLoading: false,
    playbookError: null,
    activeModuleId: null,
    streetsTier: 'playbook',
    loadPlaybookModules: mockLoadModules,
    loadPlaybookContent: mockLoadContent,
    loadPlaybookProgress: mockLoadProgress,
    markLessonComplete: mockMarkComplete,
    loadStreetsTier: vi.fn(),
    activateStreetsLicense: vi.fn(),

    // StreetHealthBadge slice
    streetHealth: null,
    loadStreetHealth: mockLoadStreetHealth,

    // Personalization slice
    personalizedLessons: {},
    loadPersonalizedContent: vi.fn(),
    loadPersonalizedContentBatch: vi.fn(),

    // Toast
    addToast: vi.fn(),

    // Browser mode (true in test env — no Tauri)
    isBrowserMode: true,

    // AWE slice
    aweSummary: null,
    awePatterns: null,
    awePendingDecisions: [],
    aweGrowthTrajectory: null,
    aweWisdomWell: null,
    aweBehavioralContext: null,
    aweWisdomSynthesis: null,
    aweLoading: false,
    aweLastSync: null,
    loadAweSummary: vi.fn(),
    loadAwePatterns: vi.fn(),
    loadAwePendingDecisions: vi.fn(),
    loadAweGrowthTrajectory: vi.fn(),
    loadAweWisdomWell: vi.fn(),
    loadBehavioralContext: vi.fn(),
    synthesizeWisdom: vi.fn(),
    submitAweBatchFeedback: vi.fn(),
    runAweAutoFeedback: vi.fn(),
  };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
(globalThis as any).__playbookTestStore = makeDefaultStore();

function setStore(overrides: Record<string, unknown> = {}) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).__playbookTestStore = { ...makeDefaultStore(), ...overrides };
}

vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      selector((globalThis as any).__playbookTestStore ?? {}),
    ),
    {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      getState: () => (globalThis as any).__playbookTestStore ?? {},
      setState: (partial: Record<string, unknown>) => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        Object.assign((globalThis as any).__playbookTestStore, partial);
      },
    },
  ),
}));

vi.mock('zustand/react/shallow', () => ({
  useShallow: (fn: unknown) => fn,
}));

vi.mock('../playbook/SovereignProfile', () => ({
  SovereignProfile: () => <div data-testid="sovereign-profile" />,
}));

vi.mock('../playbook/StreetHealthBadge', () => ({
  StreetHealthBadge: () => <div data-testid="street-health-badge" />,
}));

vi.mock('../playbook/SovereignInsightCard', () => ({
  SovereignInsightCard: () => <div data-testid="sovereign-insight-card" />,
}));

vi.mock('../playbook/SovereignConnectionBlock', () => ({
  SovereignConnectionBlock: () => <div data-testid="sovereign-connection-block" />,
}));

vi.mock('../playbook/DiffRibbon', () => ({
  DiffRibbon: () => <div data-testid="diff-ribbon" />,
}));

vi.mock('../playbook/FeedEchoBlock', () => ({
  FeedEchoBlock: () => <div data-testid="feed-echo-block" />,
}));

vi.mock('../playbook/ProgressiveRevealBanner', () => ({
  ProgressiveRevealBanner: () => <div data-testid="progressive-reveal-banner" />,
}));

vi.mock('../playbook/PersonalizationDepthIndicator', () => ({
  PersonalizationDepthIndicator: () => <div data-testid="personalization-depth" />,
}));

vi.mock('../playbook/TemplateLibrary', () => ({
  TemplateLibrary: () => <div data-testid="template-library" />,
}));

vi.mock('../../utils/playbook-markdown', () => ({
  renderMarkdown: (content: string) => content,
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { PlaybookView } from '../PlaybookView';

describe('PlaybookView', () => {
  beforeEach(() => {
    setStore();
    vi.clearAllMocks();
  });

  it('renders without crash', () => {
    render(<PlaybookView />);
    const titles = screen.getAllByText('streets:streets.title');
    expect(titles.length).toBeGreaterThanOrEqual(1);
  });

  it('loads modules and progress on mount', () => {
    render(<PlaybookView />);
    expect(mockLoadModules).toHaveBeenCalledTimes(1);
    expect(mockLoadProgress).toHaveBeenCalledTimes(1);
  });

  it('auto-opens Module S on first visit when zero progress', () => {
    // Rec #5: when a new user lands on the Playbook tab with zero
    // progress, the empty state is bypassed — Module S loads directly.
    setStore({
      playbookModules: [{ id: 'S', lesson_count: 8 }],
      playbookProgress: { overall_percentage: 0, modules: [] },
    });
    render(<PlaybookView />);
    expect(mockLoadContent).toHaveBeenCalledWith('S');
  });

  it('shows empty state when modules have not loaded yet', () => {
    // Edge case: modules array is empty (fetch in flight). The empty
    // state should render as fallback since auto-open requires modules.
    setStore({ playbookModules: [], playbookProgress: null });
    render(<PlaybookView />);
    expect(screen.getByText(/streets:streets\.emptyState\.headline/)).toBeInTheDocument();
    expect(screen.getByText('streets:streets.startWith')).toBeInTheDocument();
  });

  it('renders all 7 module buttons in the sidebar', () => {
    render(<PlaybookView />);
    const moduleIds = ['S', 'T', 'R', 'E1', 'E2', 'T2', 'S2'];
    for (const modId of moduleIds) {
      const elements = screen.getAllByText(modId);
      expect(elements.length).toBeGreaterThanOrEqual(1);
    }
  });

  it('clicking start button loads module S content', () => {
    render(<PlaybookView />);
    fireEvent.click(screen.getByText('streets:streets.startWith'));
    expect(mockLoadContent).toHaveBeenCalledWith('S');
  });

  it('shows loading skeleton when playbookLoading is true', () => {
    setStore({ playbookLoading: true });
    const { container } = render(<PlaybookView />);
    const pulseElements = container.querySelectorAll('.animate-pulse');
    expect(pulseElements.length).toBeGreaterThan(0);
  });

  it('shows error with browser-mode message when playbookError is set (no Tauri)', () => {
    setStore({ playbookError: 'Failed to load modules' });
    render(<PlaybookView />);
    // In test environment (no __TAURI_INTERNALS__), shows browser-mode message
    expect(screen.getByText('error.playbookBrowser')).toBeInTheDocument();
    // Retry button is hidden in browser mode
    expect(screen.queryByText('action.retry')).not.toBeInTheDocument();
  });

  it('renders module content when playbookContent is available', () => {
    setStore({
      activeModuleId: 'S',
      playbookContent: {
        module_id: 'S',
        title: 'Sovereignty Module',
        description: 'Learn about sovereignty.',
        lessons: [
          { title: 'Lesson 1', content: 'Lesson 1 content' },
          { title: 'Lesson 2', content: 'Lesson 2 content' },
        ],
      },
      playbookProgress: {
        overall_percentage: 50,
        modules: [{ module_id: 'S', percentage: 50, completed_lessons: [0] }],
      },
    });
    render(<PlaybookView />);

    expect(screen.getByText('Sovereignty Module')).toBeInTheDocument();
    expect(screen.getByText('Learn about sovereignty.')).toBeInTheDocument();
    expect(screen.getByText('Lesson 1')).toBeInTheDocument();
    expect(screen.getByText('Lesson 2')).toBeInTheDocument();
  });

  it('shows SovereignProfile when viewing Module S', () => {
    setStore({
      activeModuleId: 'S',
      playbookContent: {
        module_id: 'S',
        title: 'Sovereignty',
        description: 'Desc',
        lessons: [],
      },
    });
    render(<PlaybookView />);
    expect(screen.getByTestId('sovereign-profile')).toBeInTheDocument();
  });

  it('shows free forever notice for playbook tier', () => {
    setStore({ streetsTier: 'playbook' });
    render(<PlaybookView />);
    expect(screen.getByText('streets:streets.freeForever')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // Template library section
  // -------------------------------------------------------------------------
  it('shows template library when Templates sidebar button is clicked', () => {
    render(<PlaybookView />);

    // Click the Templates button in the sidebar
    fireEvent.click(screen.getByText('playbook.templates'));

    expect(screen.getByTestId('template-library')).toBeInTheDocument();
  });

  it('hides empty state when template library is shown', () => {
    render(<PlaybookView />);

    fireEvent.click(screen.getByText('playbook.templates'));

    // Empty state content should not be visible
    expect(screen.queryByText('streets:streets.startWith')).not.toBeInTheDocument();
    expect(screen.queryByText(/streets:streets\.emptyState\.headline/)).not.toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // Module switching
  // -------------------------------------------------------------------------
  it('switches from templates back to module content when a module is clicked', () => {
    render(<PlaybookView />);

    // First show templates
    fireEvent.click(screen.getByText('playbook.templates'));
    expect(screen.getByTestId('template-library')).toBeInTheDocument();

    // Click a module button — find the "S" module button in the sidebar
    const moduleButtons = screen.getAllByText('S');
    fireEvent.click(moduleButtons[0]!);

    // Templates should be hidden, module content should be loading
    expect(screen.queryByTestId('template-library')).not.toBeInTheDocument();
    expect(mockLoadContent).toHaveBeenCalledWith('S');
  });

  it('renders lesson completion toggle buttons', () => {
    setStore({
      activeModuleId: 'T',
      playbookContent: {
        module_id: 'T',
        title: 'Traction Module',
        description: 'Build traction.',
        lessons: [
          { title: 'Lesson A', content: 'Content A' },
          { title: 'Lesson B', content: 'Content B' },
        ],
      },
      playbookProgress: {
        overall_percentage: 0,
        modules: [{ module_id: 'T', percentage: 0, completed_lessons: [] }],
      },
    });
    render(<PlaybookView />);

    // Each lesson should have a toggle button
    expect(screen.getByLabelText('Mark "Lesson A" complete')).toBeInTheDocument();
    expect(screen.getByLabelText('Mark "Lesson B" complete')).toBeInTheDocument();
  });

  it('shows completed state for finished lessons', () => {
    setStore({
      activeModuleId: 'T',
      playbookContent: {
        module_id: 'T',
        title: 'Traction Module',
        description: 'Build traction.',
        lessons: [
          { title: 'Lesson A', content: 'Content A' },
          { title: 'Lesson B', content: 'Content B' },
        ],
      },
      playbookProgress: {
        overall_percentage: 50,
        modules: [{ module_id: 'T', percentage: 50, completed_lessons: [0] }],
      },
    });
    render(<PlaybookView />);

    // First lesson should show as completed (toggle to incomplete)
    expect(screen.getByLabelText('Mark "Lesson A" incomplete')).toBeInTheDocument();
    // Second lesson should show as not completed
    expect(screen.getByLabelText('Mark "Lesson B" complete')).toBeInTheDocument();
  });

  it('calls markLessonComplete when lesson toggle is clicked', () => {
    setStore({
      activeModuleId: 'R',
      playbookContent: {
        module_id: 'R',
        title: 'Resonance Module',
        description: 'Find resonance.',
        lessons: [
          { title: 'Lesson X', content: 'Content X' },
        ],
      },
      playbookProgress: {
        overall_percentage: 0,
        modules: [{ module_id: 'R', percentage: 0, completed_lessons: [] }],
      },
    });
    render(<PlaybookView />);

    fireEvent.click(screen.getByLabelText('Mark "Lesson X" complete'));
    expect(mockMarkComplete).toHaveBeenCalledWith('R', 0);
  });

  it('does not show SovereignProfile for non-S modules', () => {
    setStore({
      activeModuleId: 'T',
      playbookContent: {
        module_id: 'T',
        title: 'Traction',
        description: 'Desc',
        lessons: [],
      },
    });
    render(<PlaybookView />);
    expect(screen.queryByTestId('sovereign-profile')).not.toBeInTheDocument();
  });

  it('shows StreetHealthBadge when progress >= 15%', () => {
    // Rec #1: badge is hidden below 15% to avoid punishing new users.
    setStore({
      playbookProgress: { overall_percentage: 20, modules: [] },
    });
    render(<PlaybookView />);
    expect(screen.getByTestId('street-health-badge')).toBeInTheDocument();
  });

  it('hides StreetHealthBadge when progress is low', () => {
    setStore({
      playbookProgress: { overall_percentage: 5, modules: [] },
    });
    render(<PlaybookView />);
    expect(screen.queryByTestId('street-health-badge')).not.toBeInTheDocument();
  });

});
