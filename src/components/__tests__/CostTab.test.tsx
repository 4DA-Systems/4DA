import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

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

vi.mock('../../lib/commands', () => ({
  cmd: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { CostTab } from '../intelligence/CostTab';
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const { cmd } = await import('../../lib/commands') as any;

describe('CostTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading skeleton initially', () => {
    vi.mocked(cmd).mockReturnValue(new Promise(() => {}));
    const { container } = render(<CostTab />);
    expect(container.querySelector('.animate-pulse')).toBeInTheDocument();
  });

  it('shows empty state when no AI usage recorded', async () => {
    // Reject all commands so allSettled yields {status:'rejected'} for each,
    // leaving usage as null — triggers the empty state branch.
    vi.mocked(cmd).mockRejectedValue(new Error('no data'));
    render(<CostTab />);
    await waitFor(() => {
      expect(screen.getByText('No AI usage recorded yet')).toBeInTheDocument();
    });
    expect(screen.getByText(/AI cost tracking begins automatically/)).toBeInTheDocument();
  });

  it('shows empty state when cost is zero and no providers', async () => {
    vi.mocked(cmd).mockImplementation((command: string) => {
      if (command === 'get_ai_usage_summary') {
        return Promise.resolve({
          period: 'month', total_cost_usd: 0,
          total_tokens_in: 0, total_tokens_out: 0,
          by_provider: [], by_task: [], recommendation: null,
        });
      }
      return Promise.resolve(null);
    });

    render(<CostTab />);
    await waitFor(() => {
      expect(screen.getByText('No AI usage recorded yet')).toBeInTheDocument();
    });
  });

  it('renders cost breakdown when data is available', async () => {
    vi.mocked(cmd).mockImplementation((command: string) => {
      switch (command) {
        case 'get_ai_usage_summary':
          return Promise.resolve({
            period: 'month', total_cost_usd: 1.25,
            total_tokens_in: 500000, total_tokens_out: 120000,
            by_provider: [
              { provider: 'OpenAI', model: 'gpt-4o', cost_usd: 1.0, request_count: 20 },
              { provider: 'Ollama', model: 'llama3', cost_usd: 0.25, request_count: 50 },
            ],
            by_task: [
              { task_type: 'embedding', cost_usd: 0.8, request_count: 60, avg_tokens: 500 },
            ],
            recommendation: null,
          });
        case 'get_ai_cost_recommendation':
          return Promise.resolve(null);
        default:
          return Promise.resolve(null);
      }
    });

    render(<CostTab />);

    await waitFor(() => {
      expect(screen.getByText('Total Cost')).toBeInTheDocument();
    });
    expect(screen.getByText('Tokens In')).toBeInTheDocument();
    expect(screen.getByText('Tokens Out')).toBeInTheDocument();
    expect(screen.getByText('500.0K')).toBeInTheDocument();
    expect(screen.getByText('By Provider')).toBeInTheDocument();
    expect(screen.getByText('OpenAI')).toBeInTheDocument();
    expect(screen.getByText('gpt-4o')).toBeInTheDocument();
    expect(screen.getByText('By Task')).toBeInTheDocument();
    expect(screen.getByText('embedding')).toBeInTheDocument();
  });

  it('renders cost optimization recommendation', async () => {
    const rec = {
      current_provider: 'OpenAI', current_model: 'gpt-4o',
      recommended_provider: 'Ollama', recommended_model: 'llama3',
      estimated_savings_usd: 4.5, quality_match_pct: 85,
      reason: 'Local model handles embedding well',
    };
    vi.mocked(cmd).mockImplementation((command: string) => {
      switch (command) {
        case 'get_ai_usage_summary':
          return Promise.resolve({
            period: 'month', total_cost_usd: 5.0,
            total_tokens_in: 2000000, total_tokens_out: 500000,
            by_provider: [
              { provider: 'OpenAI', model: 'gpt-4o', cost_usd: 5.0, request_count: 100 },
            ],
            by_task: [],
            recommendation: rec,
          });
        case 'get_ai_cost_recommendation':
          return Promise.resolve(rec);
        default:
          return Promise.resolve(null);
      }
    });

    render(<CostTab />);

    await waitFor(() => {
      expect(screen.getByText('Optimization')).toBeInTheDocument();
    });
    expect(screen.getByText(/Local model handles embedding well/)).toBeInTheDocument();
    expect(screen.getByText('$4.50/mo')).toBeInTheDocument();
  });
});
