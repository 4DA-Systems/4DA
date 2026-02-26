import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SavedItemsView } from './SavedItemsView';

// Mock Tauri API
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// Mock store
const mockAddToast = vi.fn();
const mockSetFeedbackGivenFull = vi.fn();
vi.mock('../store', () => ({
  useAppStore: vi.fn((selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      addToast: mockAddToast,
      setFeedbackGivenFull: mockSetFeedbackGivenFull,
    }),
  ),
}));

describe('SavedItemsView', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockAddToast.mockReset();
    mockSetFeedbackGivenFull.mockReset();
  });

  it('renders empty state correctly', async () => {
    mockInvoke.mockResolvedValue([]);
    render(<SavedItemsView />);
    await waitFor(() => {
      expect(screen.getByText('saved.empty.title')).toBeTruthy();
    });
  });

  it('renders saved items from backend', async () => {
    mockInvoke.mockResolvedValue([
      {
        item_id: 1,
        title: 'Saved Article One',
        url: 'https://example.com/1',
        source_type: 'hackernews',
        saved_at: '2025-01-15T10:00:00',
        summary: 'A great article about Rust.',
        content_preview: null,
      },
      {
        item_id: 2,
        title: 'Saved Article Two',
        url: null,
        source_type: 'reddit',
        saved_at: '2025-01-16T12:00:00',
        summary: null,
        content_preview: 'Preview text...',
      },
    ]);
    render(<SavedItemsView />);
    await waitFor(() => {
      expect(screen.getByText('Saved Article One')).toBeTruthy();
    });
    expect(screen.getByText('Saved Article Two')).toBeTruthy();
    expect(screen.getByText('A great article about Rust.')).toBeTruthy();
    expect(screen.getByText('Preview text...')).toBeTruthy();
    expect(screen.getByText('saved.count')).toBeTruthy();
  });

  it('remove button triggers backend call', async () => {
    mockInvoke.mockResolvedValueOnce([
      {
        item_id: 42,
        title: 'To Remove',
        url: null,
        source_type: 'hackernews',
        saved_at: '2025-01-15T10:00:00',
        summary: null,
        content_preview: null,
      },
    ]);
    // Second call is the remove
    mockInvoke.mockResolvedValueOnce(undefined);

    render(<SavedItemsView />);
    await waitFor(() => {
      expect(screen.getByText('To Remove')).toBeTruthy();
    });
    fireEvent.click(screen.getByText('saved.remove'));
    // Item should be removed optimistically
    await waitFor(() => {
      expect(screen.queryByText('To Remove')).toBeNull();
    });
  });

  it('shows loading spinner initially', () => {
    mockInvoke.mockReturnValue(new Promise(() => {})); // never resolves
    render(<SavedItemsView />);
    // Spinner should be present (animate-spin class)
    const spinner = document.querySelector('.animate-spin');
    expect(spinner).toBeTruthy();
  });

  it('shows error state with retry button', async () => {
    mockInvoke.mockRejectedValue('Database error');
    render(<SavedItemsView />);
    await waitFor(() => {
      expect(screen.getByText('Database error')).toBeTruthy();
    });
    expect(screen.getByText('action.retry')).toBeTruthy();
  });
});
