import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ArticleReader } from './ArticleReader';

// Mock Tauri API
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('ArticleReader', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('renders "Read Article" button initially', () => {
    render(<ArticleReader itemId={1} />);
    expect(screen.getByText('Read Article')).toBeTruthy();
  });

  it('shows loading state when clicked', async () => {
    mockInvoke.mockReturnValue(new Promise(() => {})); // never resolves
    render(<ArticleReader itemId={1} />);
    fireEvent.click(screen.getByText('Read Article'));
    expect(screen.getByText('Loading...')).toBeTruthy();
  });

  it('displays content after successful fetch', async () => {
    mockInvoke.mockResolvedValue({
      content: 'This is the article content.',
      source_type: 'hackernews',
      word_count: 5,
      has_summary: false,
      summary: null,
    });
    render(<ArticleReader itemId={1} url="https://example.com" />);
    fireEvent.click(screen.getByText('Read Article'));
    await waitFor(() => {
      expect(screen.getByText('This is the article content.')).toBeTruthy();
    });
    expect(screen.getByText('5 words')).toBeTruthy();
    expect(screen.getByText('~1 min read')).toBeTruthy();
    expect(screen.getByText('Copy URL')).toBeTruthy();
    expect(screen.getByText('Close')).toBeTruthy();
  });

  it('shows error state on failure', async () => {
    mockInvoke.mockRejectedValue('Network error');
    render(<ArticleReader itemId={1} />);
    fireEvent.click(screen.getByText('Read Article'));
    await waitFor(() => {
      expect(screen.getByText('Network error')).toBeTruthy();
    });
    expect(screen.getByText('Retry')).toBeTruthy();
  });

  it('shows error when no content available', async () => {
    mockInvoke.mockResolvedValue({
      content: '',
      source_type: 'hackernews',
      word_count: 0,
      has_summary: false,
      summary: null,
    });
    render(<ArticleReader itemId={1} />);
    fireEvent.click(screen.getByText('Read Article'));
    await waitFor(() => {
      expect(screen.getByText('No content available for this item.')).toBeTruthy();
    });
  });

  it('displays word count and read time', async () => {
    mockInvoke.mockResolvedValue({
      content: 'A '.repeat(600),
      source_type: 'hackernews',
      word_count: 600,
      has_summary: false,
      summary: null,
    });
    render(<ArticleReader itemId={1} />);
    fireEvent.click(screen.getByText('Read Article'));
    await waitFor(() => {
      expect(screen.getByText('600 words')).toBeTruthy();
    });
    expect(screen.getByText('~3 min read')).toBeTruthy();
  });
});
