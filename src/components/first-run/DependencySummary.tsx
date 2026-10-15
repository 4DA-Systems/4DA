// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import type { AdvisorySyncState } from '../../../src-tauri/bindings/bindings/AdvisorySyncState';
import type { ScanSummary } from './utils';

/** While the check runs, ask again this often so the line moves on its own. */
export const ADVISORY_POLL_MS = 15_000;

const RUNNING: ReadonlySet<AdvisorySyncState> = new Set(['not_started', 'syncing', 'matching']);

function advisoryKey(state: AdvisorySyncState): string {
  switch (state) {
    case 'ready':
      return 'firstRun.advisory.ready';
    case 'syncing':
    case 'matching':
      return 'firstRun.advisory.checking';
    case 'failed':
      return 'firstRun.advisory.failed';
    case 'no_inventory':
      return 'firstRun.advisory.uncovered';
    case 'not_started':
    case 'stale':
      return 'firstRun.advisory.notStarted';
  }
}

/**
 * The first-run result, dependency first (fresh-profile audit E7): what the
 * scan found — projects, dependencies, ecosystems — and where the
 * vulnerability check of them stands, instead of leading with a news
 * relevance count ("0 relevant … Nothing matched yet").
 */
export function DependencySummary({ initial }: { initial: ScanSummary }) {
  const { t } = useTranslation();
  const [summary, setSummary] = useState(initial);
  const state = summary.advisory_sync?.state ?? null;
  const running = state !== null && RUNNING.has(state);

  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      void cmd('ace_get_scan_summary')
        .then((next) => {
          const s = next as unknown as ScanSummary;
          if (!cancelled && s.has_data) setSummary(s);
        })
        .catch(() => { /* keep what we have */ });
    };
    refresh();
    if (!running) return () => { cancelled = true; };
    const id = setInterval(refresh, ADVISORY_POLL_MS);
    return () => { cancelled = true; clearInterval(id); };
  }, [running]);

  const eco = summary.dependencies_by_ecosystem;
  const ecosystems = [eco.rust, eco.npm, eco.python, eco.other].filter(n => n > 0).length;
  const stats: Array<[number, string]> = [
    [summary.projects_scanned, t('firstRun.projects')],
    [summary.total_dependencies, t('firstRun.dependencies')],
    [ecosystems, t('firstRun.ecosystems')],
  ];

  return (
    <div className="mb-6 max-w-sm mx-auto" data-testid="first-run-dependencies">
      <p className="text-[10px] text-text-muted uppercase tracking-wider mb-2">
        {t('firstRun.depsTitle')}
      </p>
      <div className="flex justify-center gap-8 mb-3">
        {stats.map(([n, label]) => (
          <div key={label} className="text-center">
            <span className="text-2xl font-bold text-text-primary tabular-nums">{n}</span>
            <p className="text-[10px] text-text-muted uppercase tracking-wider mt-0.5">{label}</p>
          </div>
        ))}
      </div>
      {state && (
        <p
          className={`text-xs text-text-secondary${running ? ' animate-pulse' : ''}`}
          data-testid="first-run-advisory"
          role="status"
        >
          {t(advisoryKey(state))}
        </p>
      )}
    </div>
  );
}
