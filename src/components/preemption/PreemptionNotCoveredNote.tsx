// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import type { NotCovered } from '../../../src-tauri/bindings/bindings/NotCovered';

/** Files named in the note; the count covers the rest. */
const LISTED = 5;

/**
 * Dependency files 4DA saw but does not read installed versions from
 * (Maven, Gradle, NuGet …): named, so a clean or short worklist is never
 * read as covering them (audit A2/E4, 2026-10-12). Same `not_covered`
 * concept as the MCP server's scan answer.
 */
export const PreemptionNotCoveredNote = memo(function PreemptionNotCoveredNote({
  notCovered,
}: {
  notCovered: NotCovered | null | undefined;
}) {
  const { t } = useTranslation();
  if (!notCovered || notCovered.count + notCovered.unreadable_count === 0) return null;
  const listed = notCovered.files.slice(0, LISTED);
  const more = notCovered.count - listed.length;
  const unreadable = notCovered.unreadable.slice(0, LISTED);
  return (
    <div
      className="px-4 py-3 rounded-lg bg-bg-secondary border border-border text-xs"
      data-testid="preemption-not-covered"
      role="note"
    >
      {notCovered.count > 0 && (
        <>
          <p className="font-medium text-text-primary mb-1">
            {t('preemption.notCovered.title', { count: notCovered.count })}
          </p>
          <p className="text-text-muted mb-1.5">{t('preemption.notCovered.body')}</p>
          <ul className="space-y-0.5 text-text-secondary font-mono text-[11px]">
            {listed.map((file) => (
              <li key={file.path} className="break-all">
                {`${file.ecosystem}: ${file.path}`}
              </li>
            ))}
          </ul>
          {more > 0 && (
            <p className="text-text-muted mt-1">{t('preemption.notCovered.more', { count: more })}</p>
          )}
        </>
      )}
      {notCovered.unreadable_count > 0 && (
        <div className={notCovered.count > 0 ? 'mt-3' : undefined} data-testid="preemption-unreadable">
          <p className="font-medium text-text-primary mb-1">
            {t('preemption.notCovered.unreadableTitle', { count: notCovered.unreadable_count })}
          </p>
          <p className="text-text-muted mb-1.5">{t('preemption.notCovered.unreadableBody')}</p>
          <ul className="space-y-0.5 text-text-secondary font-mono text-[11px]">
            {unreadable.map((file) => (
              <li key={file.path} className="break-all">
                {`${file.path}: ${file.reason}`}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
});
