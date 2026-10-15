// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd, CommandTimeoutError } from '../../lib/commands';
import type { PreemptionNegative } from '../../lib/negative-readiness';
import { formatRelativeDate } from '../../utils/format-date';

/** Ask again while advisories are being checked: the post-sync recompute lands in minutes. */
export const ADVISORY_RECHECK_DELAYS_MS = [30_000, 120_000] as const;
/** After the first asks, keep asking at this pace for as long as the check runs. */
export const ADVISORY_RECHECK_INTERVAL_MS = 60_000;

type NoticeKind = Exclude<PreemptionNegative['kind'], 'assert' | 'silent'>;

/** The backend's sync time is UTC `YYYY-MM-DD HH:MM:SS`. */
function syncedAge(syncedAt: string | null): string {
  if (!syncedAt) return '';
  return formatRelativeDate(`${syncedAt.replace(' ', 'T')}Z`);
}

/** Whether a sync is running now (the "Check now" button would only queue behind it). */
function syncRunning(negative: PreemptionNegative): boolean {
  if (negative.kind === 'checking') return true;
  return (negative.kind === 'failed' || negative.kind === 'stale') && negative.inFlight;
}

/**
 * What an empty worklist says when "your stack is clean" is not yet true:
 * the advisory check has not started, is running, failed, is too old, or
 * cannot cover these dependencies. Never silence forever, never a false
 * all-clear. Where a check can help, "Check now" runs it (`osv_sync_now`).
 */
export const PreemptionAdvisoryNotice = memo(function PreemptionAdvisoryNotice({
  negative,
  onRecheck,
}: {
  negative: Extract<PreemptionNegative, { kind: NoticeKind }>;
  onRecheck: () => void;
}) {
  const { t } = useTranslation();
  const [checkingNow, setCheckingNow] = useState(false);
  const [checkError, setCheckError] = useState<string | null>(null);
  const running = syncRunning(negative) || checkingNow;

  useEffect(() => {
    if (!running) return;
    const timers = ADVISORY_RECHECK_DELAYS_MS.map((ms) => setTimeout(onRecheck, ms));
    const last = ADVISORY_RECHECK_DELAYS_MS[ADVISORY_RECHECK_DELAYS_MS.length - 1] ?? 0;
    let interval: ReturnType<typeof setInterval> | null = null;
    const start = setTimeout(() => {
      interval = setInterval(onRecheck, ADVISORY_RECHECK_INTERVAL_MS);
    }, last);
    return () => {
      timers.forEach(clearTimeout);
      clearTimeout(start);
      if (interval) clearInterval(interval);
    };
  }, [running, onRecheck]);

  const checkNow = useCallback(() => {
    setCheckingNow(true);
    setCheckError(null);
    void cmd('osv_sync_now')
      .then(() => onRecheck())
      .catch((e: unknown) => {
        // A long first download outlives the IPC timeout while the sync keeps
        // running in the backend: that is "still checking", not a failure.
        if (e instanceof CommandTimeoutError) onRecheck();
        else setCheckError(String(e));
      })
      .finally(() => setCheckingNow(false));
  }, [onRecheck]);

  const uncoveredByTemp = negative.kind === 'uncovered' && negative.tempSkipped > 0;
  const title = uncoveredByTemp
    ? t('preemption.advisories.tempSkipped.title')
    : {
        checking: t('preemption.advisories.checking.title'),
        not_started: t('preemption.advisories.notStarted.title'),
        failed: t('preemption.advisories.failed.title'),
        stale: t('preemption.advisories.stale.title'),
        uncovered: t('preemption.advisories.uncovered.title'),
      }[negative.kind];
  const body = (() => {
    switch (negative.kind) {
      case 'checking':
        return t('preemption.advisories.checking.body');
      case 'not_started':
        return t('preemption.advisories.notStarted.body');
      case 'failed':
        return t('preemption.advisories.failed.body');
      case 'stale':
        return t('preemption.advisories.stale.body', { age: syncedAge(negative.syncedAt) });
      case 'uncovered':
        return uncoveredByTemp
          ? t('preemption.advisories.tempSkipped.body', { count: negative.tempSkipped })
          : t('preemption.advisories.uncovered.body');
    }
  })();
  const canCheck = negative.kind === 'not_started' || negative.kind === 'failed' || negative.kind === 'stale';

  return (
    <div
      className="flex flex-col items-center justify-center py-16 text-center gap-2"
      data-testid={`preemption-advisories-${negative.kind}`}
      role="status"
    >
      <p className={`text-sm font-medium text-text-primary${running ? ' animate-pulse' : ''}`}>
        {title}
      </p>
      <p className="text-xs text-text-muted max-w-md">{body}</p>
      {negative.kind === 'failed' && negative.error && (
        <p className="text-[11px] text-red-300 max-w-md break-words">{negative.error}</p>
      )}
      {canCheck && (
        <button
          type="button"
          onClick={checkNow}
          disabled={running}
          data-testid="preemption-advisories-check-now"
          className="mt-2 px-3 py-1.5 text-xs bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:text-text-primary transition-colors disabled:opacity-50"
        >
          {running ? t('preemption.advisories.checkingNow') : t('preemption.advisories.checkNow')}
        </button>
      )}
      {checkError && (
        <p className="text-[11px] text-red-300 max-w-md break-words" role="alert">
          {t('preemption.advisories.checkFailed', { error: checkError })}
        </p>
      )}
    </div>
  );
});
