import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { StandingQueries } from './StandingQueries';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (opts) {
        let result = key;
        for (const [k, v] of Object.entries(opts)) {
          result = result.replace(`{{${k}}}`, String(v));
        }
        return result;
      }
      return key;
    },
  }),
}));

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const sampleWatches = [
  {
    id: 1,
    query_text: 'Rust async patterns',
    keywords: 'rust,async',
    created_at: '2026-03-01T00:00:00Z',
    last_run: '2026-03-07T00:00:00Z',
    total_matches: 15,
    new_matches: 3,
    active: true,
  },
  {
    id: 2,
    query_text: 'Tauri security',
    keywords: 'tauri,security',
    created_at: '2026-03-02T00:00:00Z',
    last_run: null,
    total_matches: 7,
    new_matches: 0,
    active: true,
  },
];

describe('StandingQueries', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('returns null when isPro is false', () => {
    const { container } = render(<StandingQueries isPro={false} />);
    expect(container.innerHTML).toBe('');
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('shows "My Watches" header', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<StandingQueries isPro={true} />);
    expect(screen.getByText('search.myWatches')).toBeInTheDocument();
  });

  it('shows empty state hint when no watches', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<StandingQueries isPro={true} />);
    await waitFor(() => {
      expect(screen.getByText('search.watchHint')).toBeInTheDocument();
    });
  });

  it('renders watch items with query text and match counts', async () => {
    mockInvoke.mockResolvedValue(sampleWatches);
    render(<StandingQueries isPro={true} />);
    await waitFor(() => {
      expect(screen.getByText('Rust async patterns')).toBeInTheDocument();
      expect(screen.getByText('Tauri security')).toBeInTheDocument();
    });
    // Total match counts
    expect(screen.getByText('15')).toBeInTheDocument();
    expect(screen.getByText('7')).toBeInTheDocument();
  });

  it('shows new matches badge when > 0', async () => {
    mockInvoke.mockResolvedValue(sampleWatches);
    render(<StandingQueries isPro={true} />);
    await waitFor(() => {
      expect(screen.getByText('+3')).toBeInTheDocument();
    });
    // The second watch has 0 new matches, so no +0 badge
    expect(screen.queryByText('+0')).not.toBeInTheDocument();
  });

  it('calls delete_standing_query on delete click', async () => {
    mockInvoke.mockResolvedValue(sampleWatches);
    render(<StandingQueries isPro={true} />);

    await waitFor(() => {
      expect(screen.getByText('Rust async patterns')).toBeInTheDocument();
    });

    // The delete invoke should succeed
    mockInvoke.mockResolvedValue(undefined);

    const deleteButtons = screen.getAllByLabelText('action.delete');
    fireEvent.click(deleteButtons[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('delete_standing_query', { id: 1 });
    });

    // After deletion, the watch should be removed from the list
    await waitFor(() => {
      expect(screen.queryByText('Rust async patterns')).not.toBeInTheDocument();
    });
  });

  it('loads watches on mount via list_standing_queries', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<StandingQueries isPro={true} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('list_standing_queries', {});
    });
  });
});
