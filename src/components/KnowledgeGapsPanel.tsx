// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { isSignalGateError } from '../utils/error-messages';
import { KnowledgeGapsScanPrompt } from './KnowledgeGapsScanPrompt';
import { KnowledgeGapCard } from './KnowledgeGapCard';
import { ProGate } from './ProGate';
import { useColdStartGate } from '../hooks/use-cold-start-gate';
import type { EvidenceItem } from '../../src-tauri/bindings/bindings/EvidenceItem';

// Module-level in-flight share: `get_knowledge_gaps` is a ~6s backend query,
// and StrictMode's double-mount fired it twice in parallel for identical
// results (2026-08-30 audit, IPC log ids 322/323). Every concurrent mount
// awaits ONE call; the slot clears on settle so a later remount refetches.
/** `total_tracked`: the dependencies the gaps were computed over (0 = none known). */
type GapsFeed = { items: EvidenceItem[]; total_tracked?: number | null };

let gapsInFlight: Promise<GapsFeed> | null = null;

function fetchKnowledgeGaps(): Promise<GapsFeed> {
  gapsInFlight ??= cmd('get_knowledge_gaps').finally(() => {
    gapsInFlight = null;
  });
  return gapsInFlight;
}

type LoadState = 'pending' | 'loaded' | 'gated' | 'failed';

/**
 * Preemption's Knowledge Gaps sub-view (AD-054): dependencies with security,
 * breaking or release news you have not read yet. Signal-tier — the backend
 * gates `get_knowledge_gaps`, and ProGate renders the upgrade path.
 */
export const KnowledgeGapsPanel = memo(function KnowledgeGapsPanel() {
  const { t } = useTranslation();
  const isColdStart = useColdStartGate();
  const [items, setItems] = useState<EvidenceItem[]>([]);
  const [state, setState] = useState<LoadState>('pending');
  const [noDependencies, setNoDependencies] = useState(false);

  const load = useCallback(async () => {
    setState('pending');
    try {
      const feed = await fetchKnowledgeGaps();
      setItems(feed.items);
      setNoDependencies(feed.total_tracked === 0);
      setState('loaded');
    } catch (e) {
      // A FAILED fetch must never masquerade as the "no gaps — you're current"
      // success state: a tier gate gets the upgrade path, anything else Retry.
      setState(isSignalGateError(e) ? 'gated' : 'failed');
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const intro = <p className="text-xs text-text-muted">{t('knowledgeGaps.subtitle')}</p>;

  if (state === 'pending') {
    return (
      <div className="space-y-4">
        {intro}
        <p className="text-sm text-text-muted animate-pulse py-12 text-center">{t('action.loading')}</p>
      </div>
    );
  }

  if (state === 'gated') {
    return (
      <div className="space-y-4">
        {intro}
        <ProGate feature={t('knowledgeGaps.feature')}>
          {/* Room for the gate's upgrade card — no fake rows under the blur. */}
          <div className="min-h-72 bg-bg-secondary rounded-lg border border-border" data-testid="knowledge-gaps-gated" />
        </ProGate>
      </div>
    );
  }

  if (state === 'failed') {
    return (
      <div className="space-y-4">
        {intro}
        <div className="bg-bg-secondary rounded-lg border border-border px-5 py-4" data-testid="knowledge-gaps-error">
          <p className="text-sm text-text-secondary">{t('knowledgeGaps.loadFailed')}</p>
          <button
            type="button"
            onClick={() => void load()}
            className="mt-3 px-3 py-1.5 text-xs text-text-primary bg-bg-tertiary border border-border rounded-lg hover:border-text-muted transition-colors"
          >
            {t('action.retry')}
          </button>
        </div>
      </div>
    );
  }

  // No dependency known: nothing was checked, so no "you're current" — the
  // scan that would let it check. Independent of fetch volume (the gate below).
  if (noDependencies && items.length === 0) {
    return (
      <div className="space-y-4">
        {intro}
        <ProGate feature={t('knowledgeGaps.feature')}>
          <KnowledgeGapsScanPrompt />
        </ProGate>
      </div>
    );
  }

  // Intelligence Doctrine Rule 6: on day one an empty result says nothing —
  // never "no gaps", which the first week of reading cannot yet support.
  if (isColdStart && items.length === 0) return intro;

  const urgentCount = items.filter(
    (it) => it.urgency === 'critical' || it.urgency === 'high',
  ).length;

  return (
    <div className="space-y-4">
      {intro}
      <ProGate feature={t('knowledgeGaps.feature')}>
        {items.length === 0 ? (
          <div className="bg-bg-secondary rounded-lg border border-border px-5 py-4 flex items-center gap-3">
            <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center shrink-0">
              {/* eslint-disable-next-line i18next/no-literal-string */}
              <span className="text-text-secondary" aria-hidden="true">✓</span>
            </div>
            <p className="text-sm text-text-secondary">{t('knowledgeGaps.noGaps', 'No gaps detected — your knowledge is current')}</p>
          </div>
        ) : (
          <section className="space-y-2" aria-label={t('knowledgeGaps.title')}>
            <p className="text-xs text-text-muted tabular-nums">
              {t('knowledgeGaps.count', { count: items.length })}
              {urgentCount > 0 && <span className="text-amber-400 ms-1">{t('knowledgeGaps.needAttention', { count: urgentCount })}</span>}
            </p>
            {items.map((it) => <KnowledgeGapCard key={it.id} item={it} />)}
          </section>
        )}
      </ProGate>
    </div>
  );
});
