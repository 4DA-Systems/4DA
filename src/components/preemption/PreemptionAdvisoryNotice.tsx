// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import type { PreemptionNegative } from '../../lib/negative-readiness';
import { formatRelativeDate } from '../../utils/format-date';

/** Ask again while advisories are being checked: the post-sync recompute lands in minutes. */
export const ADVISORY_RECHECK_DELAYS_MS = [30_000, 120_000] as const;

type NoticeKind = Exclude<PreemptionNegative['kind'], 'assert' | 'silent'>;

/** The backend's sync time is UTC `YYYY-MM-DD HH:MM:SS`. */
function syncedAge(syncedAt: string | null): string {
  if (!syncedAt) return '';
  return formatRelativeDate(`${syncedAt.replace(' ', 'T')}Z`);
}

/**
 * What an empty worklist says when "your stack is clean" is not yet true:
 * the advisory check is running, failed, is too old, or cannot cover these
 * dependencies. Never silence forever, never a false all-clear.
 */
export const PreemptionAdvisoryNotice = memo(function PreemptionAdvisoryNotice({
  negative,
  onRecheck,
}: {
  negative: Extract<PreemptionNegative, { kind: NoticeKind }>;
  onRecheck: () => void;
}) {
  const { t } = useTranslation();
  const checking = negative.kind === 'checking';

  useEffect(() => {
    if (!checking) return;
    const timers = ADVISORY_RECHECK_DELAYS_MS.map((ms) => setTimeout(onRecheck, ms));
    return () => timers.forEach(clearTimeout);
  }, [checking, onRecheck]);

  const title = {
    checking: t('preemption.advisories.checking.title'),
    failed: t('preemption.advisories.failed.title'),
    stale: t('preemption.advisories.stale.title'),
    uncovered: t('preemption.advisories.uncovered.title'),
  }[negative.kind];
  const body = {
    checking: () => t('preemption.advisories.checking.body'),
    failed: () => t('preemption.advisories.failed.body'),
    stale: () =>
      t('preemption.advisories.stale.body', {
        age: negative.kind === 'stale' ? syncedAge(negative.syncedAt) : '',
      }),
    uncovered: () => t('preemption.advisories.uncovered.body'),
  }[negative.kind]();

  return (
    <div
      className="flex flex-col items-center justify-center py-16 text-center gap-2"
      data-testid={`preemption-advisories-${negative.kind}`}
      role="status"
    >
      <p className={`text-sm font-medium text-text-primary${checking ? ' animate-pulse' : ''}`}>
        {title}
      </p>
      <p className="text-xs text-text-muted max-w-md">{body}</p>
      {negative.kind === 'failed' && negative.error && (
        <p className="text-[11px] text-red-300 max-w-md break-words">{negative.error}</p>
      )}
    </div>
  );
});
