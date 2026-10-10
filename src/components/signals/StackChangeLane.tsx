// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// Signal Lane 1, "Your stack": the deterministic stack-change stream
// (AD-054). The backend orders it (security, yanked, breaking, minor) and
// says whether any dependency was read at all. Four honest states: loading,
// error, cold start (no lockfile read: onboarding, doctrine rule 6) and
// "nothing changed" (lockfiles read, nothing to report).

import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { EvidenceFeed } from '../../../src-tauri/bindings/bindings/EvidenceFeed';
import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';
import { StackChangeCard } from './StackChangeCard';
import { STACK_LANE_CAP, laneState, releasesWithheld } from './stack-change';

type Load =
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'ready'; feed: EvidenceFeed };

const buttonClass =
  'px-3 py-1.5 text-xs bg-bg-tertiary text-text-primary border border-border rounded-lg hover:border-text-muted ' +
  'transition-colors font-medium disabled:opacity-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-gold/50';

const ColdStart = memo(function ColdStart({ onScanned }: { onScanned: () => void }) {
  const { t } = useTranslation();
  const isScanning = useAppStore((s) => s.isScanning);
  const runAutoDiscovery = useAppStore((s) => s.runAutoDiscovery);
  const loadUserContext = useAppStore((s) => s.loadUserContext);
  // The scan is the same one-click, fully-local `ace_auto_discover` the
  // Brief's personalize card runs; the click is the consent (INV-004).
  const scan = useCallback(async () => {
    await runAutoDiscovery();
    await loadUserContext();
    onScanned();
  }, [runAutoDiscovery, loadUserContext, onScanned]);
  return (
    <div data-testid="stack-lane-cold-start">
      <p className="text-sm text-text-primary font-medium">{t('signals.stack.coldTitle')}</p>
      <p className="text-xs text-text-muted mt-0.5 mb-3">{t('signals.stack.coldBody')}</p>
      <button type="button" onClick={() => void scan()} disabled={isScanning} className={buttonClass}>
        {isScanning ? t('onboarding.choice.scanning') : t('onboarding.choice.scanProjects')}
      </button>
    </div>
  );
});

export const StackChangeLane = memo(function StackChangeLane() {
  const { t } = useTranslation();
  const [load, setLoad] = useState<Load>({ status: 'loading' });
  const [showAll, setShowAll] = useState(false);
  const request = useRef(0);

  const fetchFeed = useCallback((force: boolean) => {
    const id = ++request.current;
    setLoad({ status: 'loading' });
    cmd('get_stack_changes', { force })
      .then((feed) => { if (id === request.current) setLoad({ status: 'ready', feed }); })
      .catch(() => { if (id === request.current) setLoad({ status: 'error' }); });
  }, []);

  useEffect(() => { fetchFeed(false); }, [fetchFeed]);
  const refetch = useCallback(() => fetchFeed(true), [fetchFeed]);

  const feed = load.status === 'ready' ? load.feed : null;
  const state = feed ? laneState(feed) : null;
  const items = feed?.items ?? [];
  const visible = showAll ? items : items.slice(0, STACK_LANE_CAP);

  return (
    <section
      aria-labelledby="signal-lane-stack-heading"
      data-lane="stack"
      aria-busy={load.status === 'loading'}
      className="bg-bg-secondary rounded-lg border border-border mx-4 mt-3 mb-4 px-5 py-4"
    >
      <div className="flex items-baseline gap-2 mb-3 pb-1 border-b border-emerald-500/30">
        <span aria-hidden="true">🎯</span>
        <h3 id="signal-lane-stack-heading" className="text-sm font-medium text-emerald-400">{t('signals.laneStack')}</h3>
        <span className="text-[10px] text-text-muted ms-1 hidden sm:inline flex-1">· {t('signals.laneStackSub')}</span>
        {load.status !== 'loading' && (
          <button type="button" onClick={refetch} className="ms-auto text-[10px] text-text-muted hover:text-text-secondary">
            {t('signals.stack.refresh')}
          </button>
        )}
      </div>

      {load.status === 'loading' && (
        <p className="text-xs text-text-muted" role="status">{t('signals.stack.loading')}</p>
      )}

      {load.status === 'error' && (
        <div role="alert">
          <p className="text-xs text-text-secondary mb-2">{t('signals.stack.error')}</p>
          <button type="button" onClick={refetch} className={buttonClass}>{t('signals.stack.retry')}</button>
        </div>
      )}

      {state === 'cold_start' && <ColdStart onScanned={refetch} />}

      {state === 'empty' && feed && (
        <div data-testid="stack-lane-empty">
          <p className="text-sm text-text-primary font-medium">{t('signals.stack.emptyTitle')}</p>
          <p className="text-xs text-text-muted mt-0.5">
            {releasesWithheld(feed) ? t('signals.stack.emptyBodyFree') : t('signals.stack.emptyBody')}
          </p>
        </div>
      )}

      {state === 'items' && (
        <>
          <ul id="signal-lane-stack-list" aria-label={t('signals.stack.listLabel')} className="space-y-2">
            {visible.map((item) => (
              <li key={item.id}><StackChangeCard item={item} /></li>
            ))}
          </ul>
          {items.length > STACK_LANE_CAP && (
            <button
              type="button"
              className="w-full mt-3 px-3 py-2 text-xs font-medium rounded-lg border border-border text-text-secondary hover:text-text-primary hover:bg-bg-tertiary transition-colors"
              aria-expanded={showAll}
              aria-controls="signal-lane-stack-list"
              onClick={() => setShowAll(!showAll)}
            >
              {showAll ? t('signals.laneShowFewer') : t('signals.laneShowAll', { count: items.length })}
            </button>
          )}
        </>
      )}

      {feed && state !== 'cold_start' && releasesWithheld(feed) && (
        <p className="text-[11px] text-text-muted mt-3">{t('signals.stack.releasesSignal')}</p>
      )}
    </section>
  );
});
