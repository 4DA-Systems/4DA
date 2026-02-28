/**
 * CoachView component tests
 *
 * Tests the STREETS Coach main layout: header, tier badge, session sidebar,
 * sub-tab navigation, content switching, StreetsGate overlay, and session CRUD.
 */
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
// Store mock — configurable via mockStoreOverrides
// ---------------------------------------------------------------------------
let mockStoreOverrides: Record<string, unknown> = {};

function baseMockState(): Record<string, unknown> {
  return {
    streetsTier: 'community',
    coachSessions: [],
    activeSessionId: null,
    coachLoading: false,
    loadStreetsTier: vi.fn(),
    loadCoachSessions: vi.fn(),
    loadCoachNudges: vi.fn(),
    createCoachSession: vi.fn(() => Promise.resolve('new-id')),
    deleteCoachSession: vi.fn(() => Promise.resolve()),
    setActiveSession: vi.fn(),
    activateStreetsLicense: vi.fn(() => Promise.resolve(true)),
    tier: 'free',
    trialStatus: null,
    expired: false,
    daysRemaining: 0,
    expiresAt: null,
  };
}

vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => {
      const state = { ...baseMockState(), ...mockStoreOverrides };
      return selector(state);
    }),
    {
      getState: () => ({ setShowSettings: vi.fn() }),
    },
  ),
}));

vi.mock('zustand/react/shallow', () => ({
  useShallow: (fn: unknown) => fn,
}));

// ---------------------------------------------------------------------------
// Mock sub-components (complex, tested separately)
// ---------------------------------------------------------------------------
vi.mock('./CoachChat', () => ({
  CoachChat: () => <div data-testid="coach-chat" />,
}));
vi.mock('./EngineRecommender', () => ({
  EngineRecommender: () => <div data-testid="engine-recommender" />,
}));
vi.mock('./StrategyViewer', () => ({
  StrategyViewer: () => <div data-testid="strategy-viewer" />,
}));
vi.mock('./LaunchReviewForm', () => ({
  LaunchReviewForm: () => <div data-testid="launch-review" />,
}));
vi.mock('./ProgressDashboard', () => ({
  ProgressDashboard: () => <div data-testid="progress-dashboard" />,
}));
vi.mock('./TemplateLibrary', () => ({
  TemplateLibrary: () => <div data-testid="template-library" />,
}));
vi.mock('./VideoCurriculum', () => ({
  VideoCurriculum: () => <div data-testid="video-curriculum" />,
}));

// ---------------------------------------------------------------------------
// Mock utilities
// ---------------------------------------------------------------------------
vi.mock('../../utils/briefing-parser', () => ({
  getRelativeTime: vi.fn(() => '2 hours ago'),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { CoachView } from './CoachView';
import type { CoachSession } from '../../types/coach';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function makeSession(overrides: Partial<CoachSession> = {}): CoachSession {
  return {
    id: 'sess-1',
    session_type: 'chat',
    title: 'Test Session',
    context_snapshot: null,
    created_at: '2026-01-15T10:00:00Z',
    updated_at: '2026-01-15T12:00:00Z',
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
describe('CoachView', () => {
  beforeEach(() => {
    mockStoreOverrides = {};
  });

  // ---- 1. Smoke test ----
  it('renders without crashing', () => {
    const { container } = render(<CoachView />);
    expect(container.firstChild).toBeTruthy();
  });

  // ---- 2. Title ----
  it('displays the coach title', () => {
    render(<CoachView />);
    // react-i18next mock returns key as-is
    expect(screen.getByText('coach:coach.title')).toBeInTheDocument();
  });

  // ---- 3. Tier badge ----
  it('shows the correct tier badge for community tier', () => {
    render(<CoachView />);
    expect(screen.getByText('coach:coach.tier.community')).toBeInTheDocument();
  });

  // ---- 4. Empty state ----
  it('shows empty state message when no sessions exist', () => {
    render(<CoachView />);
    expect(screen.getByText('coach:coach.noSessions')).toBeInTheDocument();
  });

  // ---- 5. New session button ----
  it('shows the new session dropdown button', () => {
    render(<CoachView />);
    expect(screen.getByText('coach:coach.newSession')).toBeInTheDocument();
  });

  // ---- 6. New session dropdown options ----
  it('shows all session type options in the dropdown when clicked', () => {
    render(<CoachView />);
    fireEvent.click(screen.getByText('coach:coach.newSession'));

    const expectedKeys = [
      'coach:coach.tab.chat',
      'coach:coach.tab.engines',
      'coach:coach.tab.strategy',
      'coach:coach.tab.launchReview',
      'coach:coach.tab.progress',
      'coach:coach.tab.templates',
      'coach:coach.tab.curriculum',
    ];

    for (const key of expectedKeys) {
      // Each key appears both in the sub-tab nav and the dropdown.
      // The dropdown items should be visible after clicking.
      const matches = screen.getAllByText(key);
      expect(matches.length).toBeGreaterThanOrEqual(2); // sidebar tab + dropdown
    }
  });

  // ---- 7. Session list ----
  it('renders session list when sessions exist', () => {
    mockStoreOverrides = {
      coachSessions: [
        makeSession({ id: 's1', title: 'Alpha Session' }),
        makeSession({ id: 's2', title: 'Beta Session', session_type: 'strategy' }),
      ],
    };
    render(<CoachView />);
    expect(screen.getByText('Alpha Session')).toBeInTheDocument();
    expect(screen.getByText('Beta Session')).toBeInTheDocument();
    // Empty state should NOT appear
    expect(screen.queryByText('coach:coach.noSessions')).not.toBeInTheDocument();
  });

  // ---- 8. Clicking a session calls setActiveSession ----
  it('calls setActiveSession when a session is clicked', () => {
    const setActiveSession = vi.fn();
    mockStoreOverrides = {
      coachSessions: [makeSession({ id: 'sess-42', title: 'Click Me', session_type: 'chat' })],
      setActiveSession,
    };
    render(<CoachView />);
    fireEvent.click(screen.getByText('Click Me'));
    expect(setActiveSession).toHaveBeenCalledWith('sess-42');
  });

  // ---- 9. Delete button on active/hovered session ----
  it('shows delete button on active session and calls deleteCoachSession', () => {
    const deleteCoachSession = vi.fn(() => Promise.resolve());
    mockStoreOverrides = {
      coachSessions: [makeSession({ id: 'del-1', title: 'To Delete' })],
      activeSessionId: 'del-1',
      deleteCoachSession,
    };
    render(<CoachView />);

    const deleteBtn = screen.getByTitle('coach:coach.deleteSession');
    expect(deleteBtn).toBeInTheDocument();
    fireEvent.click(deleteBtn);
    expect(deleteCoachSession).toHaveBeenCalledWith('del-1');
  });

  // ---- 10. Sub-tab navigation buttons are rendered ----
  it('renders all sub-tab navigation buttons', () => {
    render(<CoachView />);
    const tabKeys = [
      'coach:coach.tab.chat',
      'coach:coach.tab.engines',
      'coach:coach.tab.strategy',
      'coach:coach.tab.launchReview',
      'coach:coach.tab.progress',
      'coach:coach.tab.templates',
      'coach:coach.tab.curriculum',
    ];
    for (const key of tabKeys) {
      expect(screen.getByText(key)).toBeInTheDocument();
    }
  });

  // ---- 11. Clicking a sub-tab switches the content view ----
  it('switches content when a sub-tab is clicked', () => {
    render(<CoachView />);

    // Default is chat
    expect(screen.getByTestId('coach-chat')).toBeInTheDocument();
    expect(screen.queryByTestId('engine-recommender')).not.toBeInTheDocument();

    // Click engines tab
    fireEvent.click(screen.getByText('coach:coach.tab.engines'));
    expect(screen.getByTestId('engine-recommender')).toBeInTheDocument();
    expect(screen.queryByTestId('coach-chat')).not.toBeInTheDocument();

    // Click strategy tab
    fireEvent.click(screen.getByText('coach:coach.tab.strategy'));
    expect(screen.getByTestId('strategy-viewer')).toBeInTheDocument();

    // Click templates tab
    fireEvent.click(screen.getByText('coach:coach.tab.templates'));
    expect(screen.getByTestId('template-library')).toBeInTheDocument();

    // Click curriculum tab
    fireEvent.click(screen.getByText('coach:coach.tab.curriculum'));
    expect(screen.getByTestId('video-curriculum')).toBeInTheDocument();
  });

  // ---- 12. Default tab shows CoachChat ----
  it('shows CoachChat as the default active content', () => {
    render(<CoachView />);
    expect(screen.getByTestId('coach-chat')).toBeInTheDocument();
  });

  // ---- 13. StreetsGate overlay when tier is playbook ----
  it('shows StreetsGate overlay when tier is playbook', () => {
    mockStoreOverrides = { streetsTier: 'playbook' };
    render(<CoachView />);
    // StreetsGate renders the gate title and license key input
    expect(screen.getByText('coach:coach.gate.title')).toBeInTheDocument();
    expect(screen.getByText('coach:coach.gate.requiresLicense')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('coach:coach.gate.enterKey')).toBeInTheDocument();
  });

  // ---- 14. StreetsGate is hidden for non-playbook tiers ----
  it('does not show StreetsGate overlay when tier is community or cohort', () => {
    mockStoreOverrides = { streetsTier: 'community' };
    render(<CoachView />);
    expect(screen.queryByText('coach:coach.gate.title')).not.toBeInTheDocument();

    // Also check cohort
    mockStoreOverrides = { streetsTier: 'cohort' };
    render(<CoachView />);
    // Cohort tier badge should appear, but no gate
    expect(screen.queryByText('coach:coach.gate.title')).not.toBeInTheDocument();
  });
});
