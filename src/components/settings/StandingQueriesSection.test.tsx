// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { StandingQueriesSection } from './StandingQueriesSection';
import type { StandingQuery, StandingQuerySuggestion, StandingQueryMatch } from '../../lib/commands';

// ---------------------------------------------------------------------------
// Mocks — same idiom as SignalsPanel.test.tsx / BlindSpotsAssessSection.test.tsx
// ---------------------------------------------------------------------------

let mockIsPro = true;
vi.mock('../../hooks/use-license', () => ({
  useLicense: () => ({ isPro: mockIsPro, trialStatus: null, expired: false, daysRemaining: 30 }),
}));

vi.mock('../../store', () => ({
  useAppStore: Object.assign(
    vi.fn((selector: (s: Record<string, unknown>) => unknown) => selector({ startTrial: vi.fn() })),
    { getState: () => ({}) },
  ),
}));

const cmdMock = vi.fn((..._a: unknown[]): Promise<unknown> => Promise.resolve(undefined));
vi.mock('../../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...a) }));

// ---------------------------------------------------------------------------
// Factories
// ---------------------------------------------------------------------------

function makeQuery(overrides: Partial<StandingQuery> = {}): StandingQuery {
  return {
    id: 1,
    query_text: 'rust async runtime',
    keywords: ['rust', 'async', 'runtime'],
    created_at: '2026-08-30 00:00:00',
    last_run: '2026-08-30 01:00:00',
    total_matches: 3,
    new_matches: 1,
    active: true,
    ...overrides,
  };
}

function makeSuggestion(overrides: Partial<StandingQuerySuggestion> = {}): StandingQuerySuggestion {
  return {
    topic: 'tauri',
    reason: 'You engaged with 5 tauri items this week',
    engagement_count: 5,
    query_type: 'topic',
    ...overrides,
  };
}

function makeMatch(overrides: Partial<StandingQueryMatch> = {}): StandingQueryMatch {
  return {
    item_id: 42,
    title: 'Tokio 2.0 released',
    source_type: 'hn',
    url: 'https://example.com/tokio',
    discovered_at: '2026-08-30 02:00:00',
    ...overrides,
  };
}

/** Route the cmd mock per command name; overrides win. */
function routeCmd(routes: Record<string, (params?: unknown) => Promise<unknown>> = {}) {
  const table: Record<string, (params?: unknown) => Promise<unknown>> = {
    list_standing_queries: () => Promise.resolve([]),
    get_standing_query_suggestions: () => Promise.resolve([]),
    create_standing_query: () => Promise.resolve(1),
    delete_standing_query: () => Promise.resolve(undefined),
    get_standing_query_matches: () => Promise.resolve([]),
    ...routes,
  };
  cmdMock.mockImplementation((name: unknown, params?: unknown) => {
    const handler = table[name as string];
    return handler ? handler(params) : Promise.resolve(undefined);
  });
}

beforeEach(() => {
  mockIsPro = true;
  routeCmd();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('StandingQueriesSection — gating', () => {
  it('renders the locked state (no commands invoked) when not licensed', () => {
    mockIsPro = false;
    render(<StandingQueriesSection />);
    expect(screen.getByText('settings.standingQueries.locked.title')).toBeInTheDocument();
    expect(screen.getByText('pro.upgrade')).toBeInTheDocument();
    expect(cmdMock).not.toHaveBeenCalled();
  });

  it('routes a backend Signal-gate rejection to the locked state, not an error', async () => {
    routeCmd({
      list_standing_queries: () => Promise.reject(new Error('This feature requires 4DA Signal')),
    });
    render(<StandingQueriesSection />);
    expect(await screen.findByText('settings.standingQueries.locked.title')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});

describe('StandingQueriesSection — list', () => {
  it('renders existing queries with their keywords and counts', async () => {
    routeCmd({
      list_standing_queries: () =>
        Promise.resolve([
          makeQuery({ id: 1, query_text: 'rust async runtime' }),
          makeQuery({ id: 2, query_text: 'sqlite vector search', keywords: ['sqlite', 'vector', 'search'], new_matches: 0, total_matches: 0, last_run: null }),
        ]),
    });
    render(<StandingQueriesSection />);
    expect(await screen.findByText('rust async runtime')).toBeInTheDocument();
    expect(screen.getByText('sqlite vector search')).toBeInTheDocument();
    // Counts from the evaluator: a query with new matches shows the badge,
    // a never-run query shows the honest "checks on next cycle" note.
    expect(screen.getByText('settings.standingQueries.newMatches')).toBeInTheDocument();
    expect(screen.getByText('settings.standingQueries.neverRun')).toBeInTheDocument();
    expect(screen.getByText('settings.standingQueries.count')).toBeInTheDocument();
  });

  it('shows the inviting create-your-first-query state when the list is empty', async () => {
    render(<StandingQueriesSection />);
    expect(await screen.findByText('settings.standingQueries.emptyTitle')).toBeInTheDocument();
    expect(screen.getByText('settings.standingQueries.emptyDesc')).toBeInTheDocument();
  });
});

describe('StandingQueriesSection — create', () => {
  it('creates a query from the input and reloads the list', async () => {
    render(<StandingQueriesSection />);
    const input = await screen.findByLabelText('settings.standingQueries.inputLabel');
    fireEvent.change(input, { target: { value: 'react server components' } });
    fireEvent.click(screen.getByText('settings.standingQueries.create'));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('create_standing_query', { queryText: 'react server components' }),
    );
    // List is reloaded after a successful create (initial load + refresh).
    await waitFor(() => {
      const listCalls = cmdMock.mock.calls.filter((c) => c[0] === 'list_standing_queries');
      expect(listCalls.length).toBeGreaterThanOrEqual(2);
    });
  });

  it('keeps the create button disabled while the input is empty', async () => {
    render(<StandingQueriesSection />);
    const button = (await screen.findByText('settings.standingQueries.create')).closest('button');
    expect(button).toBeDisabled();
  });

  it('surfaces the backend max-10 rejection as the specific cap message', async () => {
    routeCmd({
      create_standing_query: () =>
        Promise.reject(new Error('Maximum of 10 active standing queries reached. Delete one to add another.')),
    });
    render(<StandingQueriesSection />);
    const input = await screen.findByLabelText('settings.standingQueries.inputLabel');
    fireEvent.change(input, { target: { value: 'one query too many' } });
    fireEvent.click(screen.getByText('settings.standingQueries.create'));
    expect(await screen.findByRole('alert')).toHaveTextContent('settings.standingQueries.maxReached');
  });
});

describe('StandingQueriesSection — suggestions', () => {
  it('creates a query with one click on a suggestion chip', async () => {
    routeCmd({
      get_standing_query_suggestions: () => Promise.resolve([makeSuggestion({ topic: 'tauri' })]),
    });
    render(<StandingQueriesSection />);
    const chip = await screen.findByLabelText('settings.standingQueries.suggestionAria');
    fireEvent.click(chip);
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('create_standing_query', { queryText: 'tauri' }),
    );
  });

  it('hides suggestions that already exist as queries', async () => {
    routeCmd({
      list_standing_queries: () => Promise.resolve([makeQuery({ query_text: 'tauri' })]),
      get_standing_query_suggestions: () => Promise.resolve([makeSuggestion({ topic: 'Tauri' })]),
    });
    render(<StandingQueriesSection />);
    await screen.findByText('tauri');
    expect(screen.queryByLabelText('settings.standingQueries.suggestionAria')).not.toBeInTheDocument();
  });
});

describe('StandingQueriesSection — delete', () => {
  it('requires confirmation, then deletes and removes the row', async () => {
    routeCmd({
      list_standing_queries: () => Promise.resolve([makeQuery({ id: 7, query_text: 'rust async runtime' })]),
    });
    render(<StandingQueriesSection />);
    await screen.findByText('rust async runtime');

    // First click arms the confirmation — nothing is deleted yet.
    fireEvent.click(screen.getByLabelText('settings.standingQueries.delete'));
    expect(cmdMock).not.toHaveBeenCalledWith('delete_standing_query', expect.anything());

    fireEvent.click(screen.getByLabelText('settings.standingQueries.confirmDelete'));
    await waitFor(() => expect(cmdMock).toHaveBeenCalledWith('delete_standing_query', { id: 7 }));
    await waitFor(() => expect(screen.queryByText('rust async runtime')).not.toBeInTheDocument());
  });

  it('cancel backs out of the confirmation without deleting', async () => {
    routeCmd({
      list_standing_queries: () => Promise.resolve([makeQuery({ id: 7 })]),
    });
    render(<StandingQueriesSection />);
    await screen.findByText('rust async runtime');
    fireEvent.click(screen.getByLabelText('settings.standingQueries.delete'));
    fireEvent.click(screen.getByLabelText('action.cancel'));
    expect(screen.getByLabelText('settings.standingQueries.delete')).toBeInTheDocument();
    expect(cmdMock).not.toHaveBeenCalledWith('delete_standing_query', expect.anything());
  });
});

describe('StandingQueriesSection — recent matches drawer', () => {
  it('lazily fetches and renders recent matches on expand', async () => {
    routeCmd({
      list_standing_queries: () => Promise.resolve([makeQuery({ id: 3 })]),
      get_standing_query_matches: () => Promise.resolve([makeMatch({ title: 'Tokio 2.0 released' })]),
    });
    render(<StandingQueriesSection />);
    await screen.findByText('rust async runtime');
    expect(cmdMock).not.toHaveBeenCalledWith('get_standing_query_matches', expect.anything());

    fireEvent.click(screen.getByLabelText('settings.standingQueries.showMatches'));
    await waitFor(() =>
      expect(cmdMock).toHaveBeenCalledWith('get_standing_query_matches', { id: 3, limit: 5 }),
    );
    expect(await screen.findByText('Tokio 2.0 released')).toBeInTheDocument();
  });
});
