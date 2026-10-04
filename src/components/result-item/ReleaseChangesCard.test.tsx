// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The release card's "what changed" block: counts, top breaking entries, the
// source link, and the honest empty states (never an invented changelog).
import { render, renderHook, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import en from '../../locales/en/ui.json';
import type { ReleaseChanges } from '../../../src-tauri/bindings/bindings/ReleaseChanges';

const strings = en as Record<string, string>;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const template = strings[key] ?? key;
      return template.replace(/\{\{(\w+)\}\}/g, (_m, name: string) => String(options?.[name] ?? ''));
    },
  }),
}));

const mockCmd = vi.fn();
vi.mock('../../lib/commands', () => ({
  cmd: (...args: unknown[]) => mockCmd(...args),
}));

import { ReleaseChangesCard } from './ReleaseChangesCard';
import { isRegistryReleaseSource, useReleaseChanges } from '../../hooks/use-release-changes';

function changes(overrides: Partial<ReleaseChanges> = {}): ReleaseChanges {
  return {
    ecosystem: 'crates.io',
    package: 'rten',
    from_version: '0.24.0',
    to_version: '0.27.0',
    status: 'found',
    file: 'rten-0.27.0/CHANGELOG.md',
    versions: ['0.27.0', '0.26.0', '0.25.0'],
    covers_range: true,
    breaking: 3,
    features: 2,
    fixes: 4,
    security: 0,
    deprecations: 0,
    other: 1,
    top_breaking: [
      { version: '0.27.0', text: '`Model::run` now returns a `Result`', under: 'Breaking changes' },
      { version: '0.25.0', text: 'Bump MSRV to 1.85', under: null },
    ],
    source_url: 'https://docs.rs/crate/rten/0.27.0/source/CHANGELOG.md',
    reason: null,
    ...overrides,
  };
}

describe('ReleaseChangesCard', () => {
  it('shows the range, the counts and the breaking entries', () => {
    render(<ReleaseChangesCard changes={changes()} loading={false} />);
    expect(screen.getByText('What changed (0.24.0 → 0.27.0)')).toBeTruthy();
    expect(screen.getByTestId('release-changes-counts').textContent).toBe(
      '3 breaking · 2 features · 4 fixes',
    );
    expect(screen.getByText('`Model::run` now returns a `Result`')).toBeTruthy();
    expect(screen.getByText('(under Breaking changes)')).toBeTruthy();
    expect(screen.getByText('Bump MSRV to 1.85')).toBeTruthy();
    expect(screen.getByText('Releases covered: 0.27.0, 0.26.0, 0.25.0')).toBeTruthy();
    const link = screen.getByText('Read the changelog');
    expect(link.getAttribute('href')).toBe('https://docs.rs/crate/rten/0.27.0/source/CHANGELOG.md');
    expect(screen.queryByText(/Partial history/)).toBeNull();
  });

  it('adds security and deprecation counts only when present', () => {
    render(<ReleaseChangesCard changes={changes({ security: 1, deprecations: 2 })} loading={false} />);
    expect(screen.getByTestId('release-changes-counts').textContent).toBe(
      '3 breaking · 2 features · 4 fixes · 1 security · 2 deprecations',
    );
  });

  it('flags a changelog that does not reach the installed version', () => {
    render(<ReleaseChangesCard changes={changes({ covers_range: false })} loading={false} />);
    expect(
      screen.getByText('Partial history: the changelog does not reach back to 0.24.0.'),
    ).toBeTruthy();
  });

  it('says there is no changelog instead of inventing one', () => {
    render(
      <ReleaseChangesCard
        changes={changes({
          status: 'no_changelog',
          file: null,
          versions: [],
          breaking: 0,
          features: 0,
          fixes: 0,
          other: 0,
          top_breaking: [],
          source_url: 'https://www.npmjs.com/package/stripe/v/23.0.0?activeTab=code',
        })}
        loading={false}
      />,
    );
    expect(screen.getByTestId('release-changes-empty').textContent).toBe('No changelog in the package.');
    expect(screen.queryByTestId('release-changes-counts')).toBeNull();
    expect(screen.getByText('View the package')).toBeTruthy();
  });

  it.each([
    ['unparsed', 'The package has a changelog, but 4DA cannot read its version headings.'],
    ['no_sections_in_range', 'The package changelog has no entry for a version after 0.24.0.'],
    ['refused', 'The package archive was not read (over the size limit or not a package archive).'],
    ['unavailable', 'The registry could not be reached. 4DA will try again.'],
  ] as const)('explains status %s', (status, text) => {
    render(<ReleaseChangesCard changes={changes({ status, top_breaking: [] })} loading={false} />);
    expect(screen.getByTestId('release-changes-empty').textContent).toBe(text);
  });

  it('hides the source link when the registry was unreachable', () => {
    render(<ReleaseChangesCard changes={changes({ status: 'unavailable' })} loading={false} />);
    expect(screen.queryByText('Read the changelog')).toBeNull();
  });

  it('renders nothing for an item with no release changes', () => {
    const { container } = render(<ReleaseChangesCard changes={null} loading={false} />);
    expect(container.innerHTML).toBe('');
  });

  it('shows a loading line while the changelog is read', () => {
    render(<ReleaseChangesCard changes={null} loading />);
    expect(screen.getByTestId('release-changes-loading').textContent).toBe('Reading the changelog…');
  });
});

describe('useReleaseChanges', () => {
  beforeEach(() => mockCmd.mockReset());

  it('asks the backend only for registry rows', async () => {
    mockCmd.mockResolvedValue(changes());
    const { result } = renderHook(() => useReleaseChanges(7, 'crates_io', true));
    await waitFor(() => expect(result.current.changes?.package).toBe('rten'));
    expect(mockCmd).toHaveBeenCalledWith('get_release_changes', { itemId: 7 });

    mockCmd.mockClear();
    const other = renderHook(() => useReleaseChanges(8, 'hackernews', true));
    expect(other.result.current.changes).toBeNull();
    expect(mockCmd).not.toHaveBeenCalled();
  });

  it('treats a failed call as no card', async () => {
    mockCmd.mockRejectedValueOnce('registry down');
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const { result } = renderHook(() => useReleaseChanges(9, 'npm_registry', true));
    await waitFor(() => expect(warn).toHaveBeenCalled());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.changes).toBeNull();
    warn.mockRestore();
  });

  it('recognises the registry source types', () => {
    expect(isRegistryReleaseSource('npm_registry')).toBe(true);
    expect(isRegistryReleaseSource('crates_io')).toBe(true);
    expect(isRegistryReleaseSource('pypi')).toBe(false);
    expect(isRegistryReleaseSource(undefined)).toBe(false);
  });
});
