// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// W0-D (fresh profile 2026-11-11): a pip-compile project read 0 of 55
// packages, four benchmark lockfiles were skipped with a log line only, and a
// ranged requirement or a manifest with no lockfile vanished uncounted, while
// Preemption read as complete. What was not checked is now named with its
// count, in the real English copy (label, never suppress).
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import en from '../../locales/en/ui.json';
import type { NotCovered } from '../../../src-tauri/bindings/bindings/NotCovered';

const strings = en as Record<string, string>;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const count = options?.count;
      const plural = typeof count === 'number' ? strings[`${key}_${count === 1 ? 'one' : 'other'}`] : undefined;
      const template = plural ?? strings[key] ?? key;
      return template.replace(/\{\{(\w+)\}\}/g, (_m, name: string) => String(options?.[name] ?? ''));
    },
  }),
}));

import { PreemptionNotCoveredNote } from './PreemptionNotCoveredNote';

const base: NotCovered = { count: 0, files: [], unreadable_count: 0, unreadable: [] };

describe('PreemptionNotCoveredNote wording (W0-D)', () => {
  it('names skipped fixture lockfiles with their reason', () => {
    render(
      <PreemptionNotCoveredNote
        notCovered={{
          ...base,
          skipped_count: 4,
          skipped: [
            { path: 'hono/benchmarks/jsx/yarn.lock', reason: 'example / fixture / scaffolding path' },
            { path: 'hono/benchmarks/routers/yarn.lock', reason: 'example / fixture / scaffolding path' },
          ],
        }}
      />,
    );
    const section = screen.getByTestId('preemption-skipped-lockfiles');
    expect(section).toHaveTextContent('4 lockfiles were found but not scanned');
    expect(section).toHaveTextContent('Nothing they pin was checked, and they are not counted as clean.');
    expect(within(section).getByText('hono/benchmarks/jsx/yarn.lock: example / fixture / scaffolding path')).toBeInTheDocument();
    // Two named, the other two counted.
    expect(section).toHaveTextContent('and 2 more');
  });

  it('counts unpinned requirements per file and manifests without a lockfile', () => {
    render(
      <PreemptionNotCoveredNote
        notCovered={{
          ...base,
          unpinned_count: 1,
          unpinned: [{ path: 'svc/requirements.txt', count: 1 }],
          manifest_only_count: 2,
          manifest_only: [
            { path: 'tool/package.json', count: 12 },
            { path: 'cli/Cargo.toml', count: 3 },
          ],
        }}
      />,
    );
    const unpinned = screen.getByTestId('preemption-unpinned');
    expect(unpinned).toHaveTextContent('1 requirement names no exact version');
    expect(unpinned).toHaveTextContent('svc/requirements.txt (1)');
    const manifests = screen.getByTestId('preemption-manifest-only');
    expect(manifests).toHaveTextContent('2 manifests have no lockfile');
    expect(manifests).toHaveTextContent('none of them was checked');
    expect(manifests).toHaveTextContent('tool/package.json (12)');
    expect(manifests).toHaveTextContent('cli/Cargo.toml (3)');
  });

  it('names a project whose dependencies have no installed version', () => {
    render(
      <PreemptionNotCoveredNote
        notCovered={{ ...base, unversioned_count: 7, unversioned: [{ path: 'c:/dev/b', count: 7 }] }}
      />,
    );
    const section = screen.getByTestId('preemption-unversioned');
    expect(section).toHaveTextContent('7 dependencies have no installed version 4DA could read');
    expect(section).toHaveTextContent('whatever other projects pin');
    expect(section).toHaveTextContent('c:/dev/b (7)');
  });

  it('a record from an older backend (no W0-D fields) still renders, and an empty one renders nothing', () => {
    const { container } = render(<PreemptionNotCoveredNote notCovered={base} />);
    expect(container).toBeEmptyDOMElement();
    render(
      <PreemptionNotCoveredNote
        notCovered={{ ...base, count: 1, files: [{ path: 'api/pom.xml', ecosystem: 'Maven' }] }}
      />,
    );
    expect(screen.getByText('Maven: api/pom.xml')).toBeInTheDocument();
    expect(screen.queryByTestId('preemption-unpinned')).toBeNull();
  });
});
