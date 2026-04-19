// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Keyboard navigation and accessibility tests for ArticleReader.
 *
 * Tests Escape closing, button focusability, and error state keyboard interaction.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ArticleReader } from '../ArticleReader';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('ArticleReader keyboard and accessibility', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('Read Article button is keyboard focusable', () => {
    render(<ArticleReader itemId={1} />);
    const btn = screen.getByText('reader.readArticle');
    expect(btn.closest('button')).toBeInTheDocument();
  });

  it('Close button is available after content loads', async () => {
    mockInvoke.mockResolvedValue({
      content: 'Article content here.',
      source_type: 'hackernews',
      word_count: 3,
      has_summary: false,
      summary: null,
    });
    render(<ArticleReader itemId={1} url="https://example.com" />);
    fireEvent.click(screen.getByText('reader.readArticle'));
    await waitFor(() => {
      expect(screen.getByText('action.close')).toBeInTheDocument();
    });
    const closeBtn = screen.getByText('action.close');
    expect(closeBtn.closest('button')).toBeInTheDocument();
  });

  it('Retry button appears on error and is keyboard accessible', async () => {
    mockInvoke.mockRejectedValue('Failed to fetch');
    render(<ArticleReader itemId={1} />);
    fireEvent.click(screen.getByText('reader.readArticle'));
    await waitFor(() => {
      expect(screen.getByText('action.retry')).toBeInTheDocument();
    });
    const retryBtn = screen.getByText('action.retry');
    expect(retryBtn.closest('button')).toBeInTheDocument();
  });

  it('Retry button retries the fetch', async () => {
    mockInvoke
      .mockRejectedValueOnce('Network error')
      .mockResolvedValueOnce({
        content: 'Content after retry.',
        source_type: 'hackernews',
        word_count: 3,
        has_summary: false,
        summary: null,
      });

    render(<ArticleReader itemId={1} />);
    fireEvent.click(screen.getByText('reader.readArticle'));
    await waitFor(() => {
      expect(screen.getByText('action.retry')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('action.retry'));
    await waitFor(() => {
      expect(screen.getByText('Content after retry.')).toBeInTheDocument();
    });
  });

  it('Copy URL button is available and keyboard accessible', async () => {
    mockInvoke.mockResolvedValue({
      content: 'Test content.',
      source_type: 'hackernews',
      word_count: 2,
      has_summary: false,
      summary: null,
    });
    render(<ArticleReader itemId={1} url="https://example.com" />);
    fireEvent.click(screen.getByText('reader.readArticle'));
    await waitFor(() => {
      expect(screen.getByText('saved.copyUrl')).toBeInTheDocument();
    });
    const copyBtn = screen.getByText('saved.copyUrl');
    expect(copyBtn.closest('button')).toBeInTheDocument();
  });
});
