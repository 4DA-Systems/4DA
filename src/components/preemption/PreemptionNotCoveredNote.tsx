// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import type { NotCovered } from '../../../src-tauri/bindings/bindings/NotCovered';

/** Files named in the note; the count covers the rest. */
const LISTED = 5;

/** One kind of unchecked dependency: a title with its count, why, and the files. */
function Section({
  testId,
  first,
  title,
  body,
  lines,
  more,
}: {
  testId: string;
  first: boolean;
  title: string;
  body: string;
  lines: string[];
  more: string | null;
}) {
  return (
    <div className={first ? undefined : 'mt-3'} data-testid={testId}>
      <p className="font-medium text-text-primary mb-1">{title}</p>
      <p className="text-text-muted mb-1.5">{body}</p>
      <ul className="space-y-0.5 text-text-secondary font-mono text-[11px]">
        {lines.map((line) => (
          <li key={line} className="break-all">
            {line}
          </li>
        ))}
      </ul>
      {more && <p className="text-text-muted mt-1">{more}</p>}
    </div>
  );
}

/** A section's lines and its "and N more" count. */
function listed<T>(items: T[] | undefined, count: number, line: (item: T) => string) {
  const shown = (items ?? []).slice(0, LISTED).map(line);
  return { lines: shown, rest: Math.max(0, count - shown.length) };
}

/**
 * Everything the advisory check did not cover, named with counts, so a
 * clean or short worklist is never read as covering it: files 4DA does not
 * read versions from (Maven, Gradle, NuGet …, audit A2/E4), lockfiles it
 * could not read, and (W0-D) lockfiles it found but left unscanned
 * (fixture, example, benchmark folders), requirements with no exact pin,
 * manifests with no lockfile, and per-project dependencies with no
 * installed version. Same policy as the MCP server's `coverage` block:
 * label, never suppress.
 */
export const PreemptionNotCoveredNote = memo(function PreemptionNotCoveredNote({
  notCovered,
}: {
  notCovered: NotCovered | null | undefined;
}) {
  const { t } = useTranslation();
  if (!notCovered) return null;
  const more = (rest: number) => (rest > 0 ? t('preemption.notCovered.more', { count: rest }) : null);
  const counted = (f: { path: string; count: number }) => `${f.path} (${f.count})`;
  const sections: Array<{ testId: string; title: string; body: string; lines: string[]; more: string | null }> = [];

  if (notCovered.count > 0) {
    const l = listed(notCovered.files, notCovered.count, (f) => `${f.ecosystem}: ${f.path}`);
    sections.push({
      testId: 'preemption-not-covered-files',
      title: t('preemption.notCovered.title', { count: notCovered.count }),
      body: t('preemption.notCovered.body'),
      lines: l.lines,
      more: more(l.rest),
    });
  }
  if (notCovered.unreadable_count > 0) {
    const l = listed(notCovered.unreadable, notCovered.unreadable_count, (f) => `${f.path}: ${f.reason}`);
    sections.push({
      testId: 'preemption-unreadable',
      title: t('preemption.notCovered.unreadableTitle', { count: notCovered.unreadable_count }),
      body: t('preemption.notCovered.unreadableBody'),
      lines: l.lines,
      more: more(l.rest),
    });
  }
  const skipped = notCovered.skipped_count ?? 0;
  if (skipped > 0) {
    const l = listed(notCovered.skipped, skipped, (f) => `${f.path}: ${f.reason}`);
    sections.push({
      testId: 'preemption-skipped-lockfiles',
      title: t('preemption.notCovered.skippedTitle', { count: skipped }),
      body: t('preemption.notCovered.skippedBody'),
      lines: l.lines,
      more: more(l.rest),
    });
  }
  const unpinned = notCovered.unpinned_count ?? 0;
  if (unpinned > 0) {
    const files = notCovered.unpinned ?? [];
    const l = listed(files, files.length, counted);
    sections.push({
      testId: 'preemption-unpinned',
      title: t('preemption.notCovered.unpinnedTitle', { count: unpinned }),
      body: t('preemption.notCovered.unpinnedBody'),
      lines: l.lines,
      more: more(l.rest),
    });
  }
  const manifests = notCovered.manifest_only_count ?? 0;
  if (manifests > 0) {
    const l = listed(notCovered.manifest_only, manifests, counted);
    sections.push({
      testId: 'preemption-manifest-only',
      title: t('preemption.notCovered.manifestOnlyTitle', { count: manifests }),
      body: t('preemption.notCovered.manifestOnlyBody'),
      lines: l.lines,
      more: more(l.rest),
    });
  }
  const unversioned = notCovered.unversioned_count ?? 0;
  if (unversioned > 0) {
    const projects = notCovered.unversioned ?? [];
    const l = listed(projects, projects.length, counted);
    sections.push({
      testId: 'preemption-unversioned',
      title: t('preemption.notCovered.unversionedTitle', { count: unversioned }),
      body: t('preemption.notCovered.unversionedBody'),
      lines: l.lines,
      more: more(l.rest),
    });
  }

  if (sections.length === 0) return null;
  return (
    <div
      className="px-4 py-3 rounded-lg bg-bg-secondary border border-border text-xs"
      data-testid="preemption-not-covered"
      role="note"
    >
      {sections.map((s, i) => (
        <Section key={s.testId} first={i === 0} {...s} />
      ))}
    </div>
  );
});
