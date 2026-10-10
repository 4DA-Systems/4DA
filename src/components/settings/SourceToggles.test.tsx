// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SourceToggles } from './SourceToggles';
import type { SourceSetting } from '../../lib/commands';

const cmdMock = vi.fn((..._a: unknown[]): Promise<unknown> => Promise.resolve(undefined));
vi.mock('../../lib/commands', () => ({ cmd: (...a: unknown[]) => cmdMock(...(a as [string, unknown])) }));

const rows: SourceSetting[] = [
  { source_type: 'osv', name: 'OSV.dev', class: 'stack', enabled: true, last_fetch: null },
  { source_type: 'crates_io', name: 'crates.io', class: 'stack', enabled: true, last_fetch: null },
  { source_type: 'hackernews', name: 'Hacker News', class: 'interest', enabled: false, last_fetch: null },
  { source_type: 'rss', name: 'RSS Feeds', class: 'interest', enabled: true, last_fetch: null },
];

beforeEach(() => {
  cmdMock.mockReset();
});

describe('SourceToggles', () => {
  it('shows a loading state, then stack sources as always on and interests as switches', async () => {
    cmdMock.mockImplementation((command: unknown) =>
      Promise.resolve(command === 'get_source_settings' ? rows : undefined),
    );
    render(<SourceToggles onStatusChange={() => {}} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
    await waitFor(() => expect(screen.getAllByRole('switch')).toHaveLength(2));
    expect(screen.getByText('OSV.dev')).toBeInTheDocument();
    expect(screen.getByText('crates.io')).toBeInTheDocument();
    const [hn, rss] = screen.getAllByRole('switch');
    expect(hn).toHaveAttribute('aria-checked', 'false');
    expect(rss).toHaveAttribute('aria-checked', 'true');
  });

  it('turns an interest on through set_source_enabled and reports it', async () => {
    const status = vi.fn();
    cmdMock.mockImplementation((command: unknown, params: unknown) => {
      if (command === 'get_source_settings') return Promise.resolve(rows);
      if (command === 'set_source_enabled') {
        const p = params as { sourceType: string; enabled: boolean };
        return Promise.resolve({ ...rows[2], enabled: p.enabled });
      }
      return Promise.resolve(undefined);
    });
    render(<SourceToggles onStatusChange={status} />);
    await waitFor(() => expect(screen.getAllByRole('switch')).toHaveLength(2));
    fireEvent.click(screen.getAllByRole('switch')[0]!);
    await waitFor(() =>
      expect(screen.getAllByRole('switch')[0]).toHaveAttribute('aria-checked', 'true'),
    );
    expect(cmdMock).toHaveBeenCalledWith('set_source_enabled', { sourceType: 'hackernews', enabled: true });
    expect(status).toHaveBeenCalledTimes(1);
  });

  it('keeps the old state and reports when the change fails', async () => {
    const status = vi.fn();
    cmdMock.mockImplementation((command: unknown) =>
      command === 'get_source_settings' ? Promise.resolve(rows) : Promise.reject(new Error('boom')),
    );
    render(<SourceToggles onStatusChange={status} />);
    await waitFor(() => expect(screen.getAllByRole('switch')).toHaveLength(2));
    fireEvent.click(screen.getAllByRole('switch')[0]!);
    await waitFor(() => expect(status).toHaveBeenCalledTimes(1));
    expect(screen.getAllByRole('switch')[0]).toHaveAttribute('aria-checked', 'false');
  });

  it('shows an error with a retry when the list cannot load', async () => {
    cmdMock.mockImplementationOnce(() => Promise.reject(new Error('db locked')));
    cmdMock.mockImplementation(() => Promise.resolve(rows));
    render(<SourceToggles onStatusChange={() => {}} />);
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button'));
    await waitFor(() => expect(screen.getAllByRole('switch')).toHaveLength(2));
  });

  it('shows the empty state when the build lists no sources', async () => {
    cmdMock.mockImplementation(() => Promise.resolve([]));
    render(<SourceToggles onStatusChange={() => {}} />);
    await waitFor(() => expect(screen.queryByRole('status')).not.toBeInTheDocument());
    expect(screen.queryAllByRole('switch')).toHaveLength(0);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
