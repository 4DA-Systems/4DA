import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';

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

// Mock error messages util
vi.mock('../utils/error-messages', () => ({
  translateError: (e: unknown) => String(e),
}));

// ---------------------------------------------------------------------------
// Component under test
// ---------------------------------------------------------------------------
import { SourceConfigPanel } from './SourceConfigPanel';

const mockInvoke = vi.mocked(invoke);

describe('SourceConfigPanel', () => {
  const onStatusChange = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();

    // Default mock: return empty sources
    mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'get_rss_feeds':
          return Promise.resolve({ feeds: [], count: 0 });
        case 'get_youtube_channels':
          return Promise.resolve({ channels: [], count: 0 });
        case 'get_twitter_handles':
          return Promise.resolve({ handles: [], count: 0 });
        case 'has_x_api_key':
          return Promise.resolve(false);
        case 'get_github_languages':
          return Promise.resolve({ languages: [], count: 0 });
        default:
          return Promise.resolve({});
      }
    });
  });

  it('renders without crash', async () => {
    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.title')).toBeInTheDocument();
    });
  });

  it('shows custom source count badge', async () => {
    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.customCount')).toBeInTheDocument();
    });
  });

  it('shows collapsed state by default with expand indicator', async () => {
    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.subtitle')).toBeInTheDocument();
    });

    // RSS fields should not be visible when collapsed
    expect(screen.queryByText('sources.rss.label')).not.toBeInTheDocument();
  });

  it('expands on click to show all source sections', async () => {
    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.title')).toBeInTheDocument();
    });

    // Click the expand button
    fireEvent.click(screen.getByText('sources.title'));

    // All source section labels should now appear
    expect(screen.getByText('sources.rss.label')).toBeInTheDocument();
    expect(screen.getByText('sources.youtube.label')).toBeInTheDocument();
    expect(screen.getByText('sources.github.label')).toBeInTheDocument();
    expect(screen.getByText('sources.twitter.label')).toBeInTheDocument();
  });

  it('shows empty state messages for each section when no sources configured', async () => {
    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.title')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('sources.title'));

    expect(screen.getByText('sources.rss.empty')).toBeInTheDocument();
    expect(screen.getByText('sources.youtube.defaultChannels')).toBeInTheDocument();
    expect(screen.getByText('sources.twitter.defaultHandles')).toBeInTheDocument();
  });

  it('shows RSS feeds when they are loaded', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_rss_feeds')
        return Promise.resolve({ feeds: ['https://blog.rust-lang.org/feed.xml'], count: 1 });
      if (cmd === 'get_youtube_channels')
        return Promise.resolve({ channels: [], count: 0 });
      if (cmd === 'get_twitter_handles')
        return Promise.resolve({ handles: [], count: 0 });
      if (cmd === 'has_x_api_key')
        return Promise.resolve(false);
      if (cmd === 'get_github_languages')
        return Promise.resolve({ languages: [], count: 0 });
      return Promise.resolve({});
    });

    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.title')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('sources.title'));

    await waitFor(() => {
      expect(screen.getByText('https://blog.rust-lang.org/feed.xml')).toBeInTheDocument();
    });
  });

  it('shows Twitter needs-key warning when no X API key is set', async () => {
    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.title')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('sources.title'));

    expect(screen.getByText('sources.twitter.needsKey')).toBeInTheDocument();
  });

  it('shows key-set indicator when X API key exists', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_rss_feeds')
        return Promise.resolve({ feeds: [], count: 0 });
      if (cmd === 'get_youtube_channels')
        return Promise.resolve({ channels: [], count: 0 });
      if (cmd === 'get_twitter_handles')
        return Promise.resolve({ handles: [], count: 0 });
      if (cmd === 'has_x_api_key')
        return Promise.resolve(true);
      if (cmd === 'get_github_languages')
        return Promise.resolve({ languages: [], count: 0 });
      return Promise.resolve({});
    });

    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.title')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('sources.title'));

    await waitFor(() => {
      expect(screen.getByText('sources.twitter.keySet')).toBeInTheDocument();
    });
  });

  it('has Add buttons for each source type', async () => {
    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.title')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('sources.title'));

    // There should be 4 "Add" buttons (RSS, YouTube, GitHub, Twitter)
    const addButtons = screen.getAllByText('action.add');
    expect(addButtons.length).toBe(4);
  });

  it('shows GitHub languages when loaded', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_rss_feeds')
        return Promise.resolve({ feeds: [], count: 0 });
      if (cmd === 'get_youtube_channels')
        return Promise.resolve({ channels: [], count: 0 });
      if (cmd === 'get_twitter_handles')
        return Promise.resolve({ handles: [], count: 0 });
      if (cmd === 'has_x_api_key')
        return Promise.resolve(false);
      if (cmd === 'get_github_languages')
        return Promise.resolve({ languages: ['rust', 'typescript'], count: 2 });
      return Promise.resolve({});
    });

    render(<SourceConfigPanel onStatusChange={onStatusChange} />);
    await waitFor(() => {
      expect(screen.getByText('sources.title')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('sources.title'));

    await waitFor(() => {
      expect(screen.getByText('rust')).toBeInTheDocument();
      expect(screen.getByText('typescript')).toBeInTheDocument();
    });
  });
});
