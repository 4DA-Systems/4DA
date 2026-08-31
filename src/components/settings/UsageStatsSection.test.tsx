// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// UsageStatsSection — the truthful AI usage panel.
//
// Regression anchor (2026-08-31 cost audit): this section used to render
// `settings.usage`, the RERANK-ONLY ledger, as "cost today" — under-reporting
// real spend by an order of magnitude once usage recording was split per
// feature. These tests pin the panel to the global ledger (`get_llm_usage`)
// and the per-feature month summary (`get_ai_usage_summary`).
import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    // Mirror the two real call shapes used by the component:
    // t(key, 'default string') and t(key, { defaultValue }).
    t: (key: string, opts?: string | Record<string, unknown>) =>
      typeof opts === 'string' ? opts : ((opts?.defaultValue as string) ?? key),
  }),
}));
vi.mock('../../lib/commands', () => ({ cmd: vi.fn() }));

const { cmd } = (await import('../../lib/commands')) as unknown as {
  cmd: ReturnType<typeof vi.fn>;
};
const { UsageStatsSection } = await import('./UsageStatsSection');

const SETTINGS = {
  // Deliberately absurd rerank-ledger values: if any of these render, the
  // panel has regressed to the rerank-only ledger.
  usage: { tokens_today: 99999999, cost_today_cents: 99999, tokens_total: 1, items_reranked: 777 },
} as never;

function mockUsage({
  daily = {
    used: 131687,
    limit: 2000000,
    limit_reached: false,
    unlimited: false,
    cost_used_cents: 48,
    cost_limit_cents: 150,
    cost_limit_reached: false,
  },
  summary = {
    period: '2026-08',
    total_cost_usd: 12.34,
    total_tokens_in: 1,
    total_tokens_out: 1,
    by_provider: [],
    by_task: [
      { task_type: 'rerank_judge', cost_usd: 2.1, request_count: 300, avg_tokens: 400 },
      { task_type: 'ingest_judge', cost_usd: 5.5, request_count: 900, avg_tokens: 300 },
      { task_type: 'some_future_tag', cost_usd: 0.4, request_count: 3, avg_tokens: 100 },
    ],
    recommendation: null,
  },
} = {}) {
  cmd.mockImplementation((name: string) => {
    if (name === 'get_llm_usage') return Promise.resolve(daily);
    if (name === 'get_ai_usage_summary') return Promise.resolve(summary);
    return Promise.reject(new Error(`unexpected cmd ${name}`));
  });
}

beforeEach(() => {
  cmd.mockReset();
});

describe('UsageStatsSection', () => {
  it('renders the GLOBAL daily ledger and cap, never the rerank-only ledger', async () => {
    mockUsage();
    render(<UsageStatsSection settings={SETTINGS} provider="anthropic" />);

    await waitFor(() => expect(screen.getByText('$0.48')).toBeInTheDocument());
    expect(screen.getByText('131,687')).toBeInTheDocument();
    expect(screen.getByText('$12.34')).toBeInTheDocument();

    // The rerank-only ledger's numbers must be nowhere in the panel.
    expect(screen.queryByText('99,999,999')).toBeNull();
    expect(screen.queryByText('$999.99')).toBeNull();
    expect(screen.queryByText('777')).toBeNull();
  });

  it('lists month spend by feature, sorted by cost, unknown tags shown raw', async () => {
    mockUsage();
    render(<UsageStatsSection settings={SETTINGS} provider="anthropic" />);

    await waitFor(() => expect(screen.getByText('$5.50')).toBeInTheDocument());
    const rows = screen.getAllByText(/^\$\d+\.\d{2}$/).map((el) => el.textContent);
    // ingest_judge ($5.50) must precede rerank_judge ($2.10).
    expect(rows.indexOf('$5.50')).toBeLessThan(rows.indexOf('$2.10'));
    // An unrecognized task_type falls back to its raw tag — spend is never hidden.
    expect(screen.getByText('some_future_tag')).toBeInTheDocument();
  });

  it('keeps the not-tracked affordance for openai-compatible providers', async () => {
    mockUsage();
    render(<UsageStatsSection settings={SETTINGS} provider="openai-compatible" />);

    await waitFor(() => expect(screen.getByText('131,687')).toBeInTheDocument());
    expect(screen.getByText('Not tracked for this provider')).toBeInTheDocument();
    expect(screen.queryByText('$0.48')).toBeNull();
  });

  it('renders placeholders, not stale numbers, when the ledger fetch fails', async () => {
    cmd.mockImplementation(() => Promise.reject(new Error('ipc down')));
    render(<UsageStatsSection settings={SETTINGS} provider="anthropic" />);

    await waitFor(() => expect(screen.getAllByText('—').length).toBeGreaterThan(0));
    expect(screen.queryByText('99,999,999')).toBeNull();
  });
});
