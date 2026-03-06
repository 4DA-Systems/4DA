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
// Store mock
// ---------------------------------------------------------------------------
const mockLoadModules = vi.fn();
const mockLoadContent = vi.fn();
const mockLoadProgress = vi.fn();
const mockMarkComplete = vi.fn();
const mockLoadStreetHealth = vi.fn();
const mockLoadSunsModules = vi.fn();

const defaultStoreState: Record<string, unknown> = {
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

  // StreetHealthBadge slice
  streetHealth: null,
  loadStreetHealth: mockLoadStreetHealth,

  // Personalization slice
  personalizedLessons: {},
  loadPersonalizedContent: vi.fn(),

  // Toast
  addToast: vi.fn(),

  // SunsDashboard slice
  sunsModules: [],
  sunsAlerts: [],
  sunsLoading: false,
  loadSunsModules: mockLoadSunsModules,
  ackSunAlert: vi.fn(),
  executeSunRecommendation: vi.fn(),
};

let currentStore = { ...defaultStoreState };

vi.mock('../../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
    selector(currentStore),
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

vi.mock('../playbook/SunsDashboard', () => ({
  SunsDashboard: () => <div data-testid="suns-dashboard" />,
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
    currentStore = { ...defaultStoreState };
    vi.clearAllMocks();
  });

  it('renders without crash', () => {
    render(<PlaybookView />);
    // The STREETS title appears in both the sidebar and the empty state
    const titles = screen.getAllByText('streets:streets.title');
    expect(titles.length).toBeGreaterThanOrEqual(1);
  });

  it('loads modules and progress on mount', () => {
    render(<PlaybookView />);
    expect(mockLoadModules).toHaveBeenCalledTimes(1);
    expect(mockLoadProgress).toHaveBeenCalledTimes(1);
  });

  it('shows empty state with start button when no module is selected', () => {
    render(<PlaybookView />);
    // The description text spans multiple i18n keys in a single <p>,
    // so use a substring matcher.
    expect(screen.getByText(/streets:streets\.selectModuleDescription/)).toBeInTheDocument();
    expect(screen.getByText('streets:streets.startWith')).toBeInTheDocument();
  });

  it('renders all 7 module buttons in the sidebar', () => {
    render(<PlaybookView />);
    // Module IDs appear as text inside badge spans in sidebar buttons.
    // Some like 'S' also appear in the empty state, so use getAllByText.
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

  it('shows loading spinner when playbookLoading is true', () => {
    currentStore = { ...defaultStoreState, playbookLoading: true };
    const { container } = render(<PlaybookView />);
    // Spinner has animate-spin class
    const spinner = container.querySelector('.animate-spin');
    expect(spinner).toBeInTheDocument();
  });

  it('shows error message when playbookError is set', () => {
    currentStore = { ...defaultStoreState, playbookError: 'Failed to load modules' };
    render(<PlaybookView />);
    expect(screen.getByText('Failed to load modules')).toBeInTheDocument();
  });

  it('renders module content when playbookContent is available', () => {
    currentStore = {
      ...defaultStoreState,
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
    };
    render(<PlaybookView />);

    expect(screen.getByText('Sovereignty Module')).toBeInTheDocument();
    expect(screen.getByText('Learn about sovereignty.')).toBeInTheDocument();
    expect(screen.getByText('Lesson 1')).toBeInTheDocument();
    expect(screen.getByText('Lesson 2')).toBeInTheDocument();
  });

  it('shows SovereignProfile when viewing Module S', () => {
    currentStore = {
      ...defaultStoreState,
      activeModuleId: 'S',
      playbookContent: {
        module_id: 'S',
        title: 'Sovereignty',
        description: 'Desc',
        lessons: [],
      },
    };
    render(<PlaybookView />);
    expect(screen.getByTestId('sovereign-profile')).toBeInTheDocument();
  });

  it('shows coaching upgrade nudge for playbook tier', () => {
    currentStore = { ...defaultStoreState, streetsTier: 'playbook' };
    render(<PlaybookView />);
    expect(screen.getByText('streets:streets.wantCoaching')).toBeInTheDocument();
  });
});
