// SPDX-License-Identifier: FSL-1.1-Apache-2.0
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
const mockLoadDecisions = vi.fn();
const mockRecordDecision = vi.fn(() => Promise.resolve());
const mockUpdateDecision = vi.fn(() => Promise.resolve());

const defaultStoreState: Record<string, unknown> = {
  decisions: [],
  decisionsLoading: false,
  decisionsError: null,
  loadDecisions: mockLoadDecisions,
  recordDecision: mockRecordDecision,
  updateDecision: mockUpdateDecision,
};

let currentStore = { ...defaultStoreState };

vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
    selector(currentStore),
  ),
}));

vi.mock('zustand/react/shallow', () => ({
  useShallow: (fn: unknown) => fn,
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { DecisionMemory } from './DecisionMemory';

function makeDecision(overrides = {}) {
  return {
    id: 1,
    decision_type: 'tech_choice',
    subject: 'Use Tauri over Electron',
    decision: 'Tauri is the better choice for privacy-first apps.',
    rationale: 'Smaller binary, no Chromium bundling, Rust backend.',
    alternatives_rejected: ['Electron', 'Flutter'],
    context_tags: ['desktop', 'architecture'],
    confidence: 0.9,
    status: 'active',
    superseded_by: null,
    created_at: '2025-01-01T00:00:00Z',
    updated_at: '2025-01-01T00:00:00Z',
    ...overrides,
  };
}

describe('DecisionMemory', () => {
  beforeEach(() => {
    currentStore = { ...defaultStoreState };
    vi.clearAllMocks();
  });

  it('renders without crash', () => {
    render(<DecisionMemory />);
    expect(screen.getByText('decisions.title')).toBeInTheDocument();
  });

  it('loads decisions on mount', () => {
    render(<DecisionMemory />);
    expect(mockLoadDecisions).toHaveBeenCalledTimes(1);
  });

  it('shows empty state when no decisions exist', () => {
    render(<DecisionMemory />);
    expect(screen.getByText('decisions.noDecisions')).toBeInTheDocument();
    expect(screen.getByText('decisions.noDecisionsHint')).toBeInTheDocument();
  });

  it('shows loading skeleton when decisionsLoading is true', () => {
    currentStore = { ...defaultStoreState, decisionsLoading: true };
    const { container } = render(<DecisionMemory />);
    const pulseElements = container.querySelectorAll('.animate-pulse');
    expect(pulseElements.length).toBeGreaterThan(0);
  });

  it('shows error with retry button when decisionsError is set', () => {
    currentStore = { ...defaultStoreState, decisionsError: 'Network error' };
    render(<DecisionMemory />);
    expect(screen.getByText('error.generic')).toBeInTheDocument();
    expect(screen.getByText('action.retry')).toBeInTheDocument();
  });

  it('renders grouped decisions by type', () => {
    currentStore = {
      ...defaultStoreState,
      decisions: [
        makeDecision({ id: 1, decision_type: 'tech_choice', subject: 'Use Tauri' }),
        makeDecision({ id: 2, decision_type: 'architecture', subject: 'Monorepo' }),
      ],
    };
    render(<DecisionMemory />);

    // Both decision subjects should appear
    expect(screen.getByText('Use Tauri')).toBeInTheDocument();
    expect(screen.getByText('Monorepo')).toBeInTheDocument();

    // Type group labels should appear (i18n keys via passthrough mock)
    expect(screen.getByText('decisions.type.tech_choice')).toBeInTheDocument();
    expect(screen.getByText('decisions.type.architecture')).toBeInTheDocument();
  });

  it('shows record count in header', () => {
    currentStore = {
      ...defaultStoreState,
      decisions: [makeDecision({ id: 1 }), makeDecision({ id: 2 })],
    };
    render(<DecisionMemory />);
    expect(screen.getByText('decisions.recorded')).toBeInTheDocument();
  });

  it('toggles the new decision form on button click', () => {
    render(<DecisionMemory />);

    // Initially the form is hidden
    expect(screen.queryByPlaceholderText('decisions.subject')).not.toBeInTheDocument();

    // Click the "Record" button
    fireEvent.click(screen.getByText('decisions.record'));

    // Form should now be visible
    expect(screen.getByPlaceholderText('decisions.subject')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('decisions.whatDecided')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('decisions.rationaleOptional')).toBeInTheDocument();
  });

  it('shows cancel text when form is open', () => {
    render(<DecisionMemory />);
    fireEvent.click(screen.getByText('decisions.record'));
    expect(screen.getByText('action.cancel')).toBeInTheDocument();
  });

  it('expands a decision to show details on click', () => {
    const decision = makeDecision({
      rationale: 'Smaller binary and better security model.',
      alternatives_rejected: ['Electron'],
      context_tags: ['desktop'],
    });
    currentStore = { ...defaultStoreState, decisions: [decision] };
    render(<DecisionMemory />);

    // Click the decision row to expand it
    fireEvent.click(screen.getByText('Use Tauri over Electron'));

    // Expanded details should show
    expect(screen.getByText('decisions.decision')).toBeInTheDocument();
    expect(screen.getByText('decisions.rationale')).toBeInTheDocument();
    expect(screen.getByText('Smaller binary and better security model.')).toBeInTheDocument();
    expect(screen.getByText('Electron')).toBeInTheDocument();
    expect(screen.getByText('desktop')).toBeInTheDocument();
  });

  it('shows Reconsider and Supersede actions for active decisions', () => {
    currentStore = {
      ...defaultStoreState,
      decisions: [makeDecision({ status: 'active' })],
    };
    render(<DecisionMemory />);

    // Expand the decision
    fireEvent.click(screen.getByText('Use Tauri over Electron'));

    expect(screen.getByText('decisions.reconsider')).toBeInTheDocument();
    expect(screen.getByText('decisions.supersede')).toBeInTheDocument();
  });

  it('shows Reaffirm and Supersede actions for reconsidering decisions', () => {
    currentStore = {
      ...defaultStoreState,
      decisions: [makeDecision({ status: 'reconsidering' })],
    };
    render(<DecisionMemory />);

    // Expand
    fireEvent.click(screen.getByText('Use Tauri over Electron'));

    expect(screen.getByText('decisions.reaffirm')).toBeInTheDocument();
    expect(screen.getByText('decisions.supersede')).toBeInTheDocument();
  });

  it('shows confidence percentage for each decision', () => {
    currentStore = {
      ...defaultStoreState,
      decisions: [makeDecision({ confidence: 0.9 })],
    };
    render(<DecisionMemory />);
    expect(screen.getByText('90%')).toBeInTheDocument();
  });
});
