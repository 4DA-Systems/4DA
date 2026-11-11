// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd, CommandTimeoutError } from '../../lib/commands';
import type { PreemptionNegative } from '../../lib/negative-readiness';
import { translateError } from '../../utils/error-messages';
import { formatRelativeDate } from '../../utils/format-date';

/**
 * A surface re-asks when the window comes back into view, at most this often.
 * 4DA lives in the tray: a tab can stay mounted for days, and the worklist
 * used to load once on mount and then show that answer for as long as it
 * stayed open.
 */
export const VISIBLE_REFRESH_MIN_GAP_MS = 30_000;

/** Call `refresh` when the window is shown or focused again (throttled). */
export function useRefreshWhenVisible(refresh: () => Promise<void> | void): void {
  useEffect(() => {
    let last = Date.now();
    const onVisible = () => {
      if (document.visibilityState !== 'visible') return;
      const now = Date.now();
      if (now - last < VISIBLE_REFRESH_MIN_GAP_MS) return;
      last = now;
      void refresh();
    };
    window.addEventListener('focus', onVisible);
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      window.removeEventListener('focus', onVisible);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, [refresh]);
}

/**
 * The worklist's "as of": when its advisory data was last verified (said
 * only while that is current; a failed or out-of-date check says its own
 * age in the notice) and a Refresh control.
 */
export const PreemptionFreshnessRow = memo(function PreemptionFreshnessRow({
  checkedAt,
  refreshing,
  onRefresh,
}: {
  /** UTC `YYYY-MM-DD HH:MM:SS`, or null when there is no current check to date. */
  checkedAt: string | null | undefined;
  refreshing: boolean;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-end gap-3 text-[11px] text-text-muted" data-testid="preemption-freshness">
      {checkedAt && (
        <span data-testid="preemption-checked-at">
          {t('preemption.advisories.asOf', { age: syncedAge(checkedAt) })}
        </span>
      )}
      <button
        type="button"
        onClick={onRefresh}
        disabled={refreshing}
        data-testid="preemption-refresh"
        className="hover:text-text-secondary transition-colors disabled:opacity-50"
      >
        {refreshing ? t('preemption.refreshing') : t('preemption.refresh')}
      </button>
    </div>
  );
});

/** Ask again while advisories are being checked: the post-sync recompute lands in minutes. */
export const ADVISORY_RECHECK_DELAYS_MS = [30_000, 120_000] as const;
/** After the first asks, keep asking at this pace for as long as the check runs. */
export const ADVISORY_RECHECK_INTERVAL_MS = 60_000;

type NoticeKind = Exclude<PreemptionNegative['kind'], 'assert' | 'silent'>;
export type AdvisoryNotice = Extract<PreemptionNegative, { kind: NoticeKind }>;

/**
 * Where the notice sits:
 * - `empty`: in place of an empty worklist (centered).
 * - `inline`: in place of another surface's empty state (Signal's "Your stack").
 * - `line`: one line above a list that HAS items, for a failed or out-of-date
 *   check. The findings below it are real, but they were matched against
 *   data that could not be confirmed current, and the reader must know.
 */
export type AdvisoryNoticeVariant = 'empty' | 'inline' | 'line';

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

/** Whether a notice has something to say above a list with items. */
export function needsAdvisoryLine(
  negative: PreemptionNegative,
): negative is Extract<PreemptionNegative, { kind: 'failed' | 'stale' }> {
  return negative.kind === 'failed' || negative.kind === 'stale';
}

const CHECK_BUTTON =
  'px-3 py-1.5 text-xs bg-bg-tertiary text-text-secondary border border-border rounded-lg ' +
  'hover:text-text-primary transition-colors disabled:opacity-50';

/**
 * What a surface says when "your stack is clean" is not yet true: the
 * advisory check has not started, is running, failed, is too old, or cannot
 * cover these dependencies. Never silence forever, never a false all-clear.
 * Where a check can help, "Check now" runs it (`osv_sync_now`).
 */
export const PreemptionAdvisoryNotice = memo(function PreemptionAdvisoryNotice({
  negative,
  onRecheck,
  variant = 'empty',
}: {
  negative: AdvisoryNotice;
  onRecheck: () => void;
  variant?: AdvisoryNoticeVariant;
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
        // The backend rejects with an error object; `String(e)` printed
        // "[object Object]" where the reason belongs.
        else setCheckError(translateError(e));
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
        // The check failed, but the data it could not refresh has an age.
        return negative.syncedAt
          ? t('preemption.advisories.failed.bodyDated', { age: syncedAge(negative.syncedAt) })
          : t('preemption.advisories.failed.body');
      case 'stale':
        return t('preemption.advisories.stale.body', { age: syncedAge(negative.syncedAt) });
      case 'uncovered':
        return uncoveredByTemp
          ? t('preemption.advisories.tempSkipped.body', { count: negative.tempSkipped })
          : t('preemption.advisories.uncovered.body');
    }
  })();
  const canCheck = negative.kind === 'not_started' || negative.kind === 'failed' || negative.kind === 'stale';
  const checkButton = canCheck && (
    <button
      type="button"
      onClick={checkNow}
      disabled={running}
      data-testid="preemption-advisories-check-now"
      className={CHECK_BUTTON}
    >
      {running ? t('preemption.advisories.checkingNow') : t('preemption.advisories.checkNow')}
    </button>
  );
  const checkFailed = checkError && (
    <p className="text-[11px] text-red-300 max-w-md break-words" role="alert">
      {t('preemption.advisories.checkFailed', { error: checkError })}
    </p>
  );

  if (variant === 'line') {
    const line = (() => {
      if (negative.kind === 'stale') {
        return t('preemption.advisories.line.stale', { age: syncedAge(negative.syncedAt) });
      }
      if (negative.kind === 'failed' && negative.syncedAt) {
        return t('preemption.advisories.line.failed', { age: syncedAge(negative.syncedAt) });
      }
      if (negative.kind === 'failed') return t('preemption.advisories.line.failedUndated');
      return body;
    })();
    return (
      <div
        className="px-4 py-2.5 rounded-lg bg-amber-500/10 border border-amber-500/20"
        data-testid="preemption-advisories-line"
        data-kind={negative.kind}
        role="status"
      >
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <p
            className="flex-1 min-w-[16rem] text-xs text-amber-400"
            title={negative.kind === 'failed' ? negative.error ?? undefined : undefined}
          >
            {line}
          </p>
          {checkButton}
        </div>
        {checkFailed}
      </div>
    );
  }

  const layout = variant === 'inline'
    ? 'flex flex-col items-start gap-2'
    : 'flex flex-col items-center justify-center py-16 text-center gap-2';
  return (
    <div className={layout} data-testid={`preemption-advisories-${negative.kind}`} role="status">
      <p className={`text-sm font-medium text-text-primary${running ? ' animate-pulse' : ''}`}>
        {title}
      </p>
      <p className="text-xs text-text-muted max-w-md">{body}</p>
      {negative.kind === 'failed' && negative.error && (
        <p className="text-[11px] text-red-300 max-w-md break-words">{negative.error}</p>
      )}
      {canCheck && <div className="mt-2">{checkButton}</div>}
      {checkFailed}
    </div>
  );
});
