// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { lazy, memo, Suspense, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../../store';
import { useLicense } from '../../hooks/use-license';
import { trackEvent } from '../../hooks/use-telemetry';
import type { PreemptionSubView } from '../../store/types';
import { ViewErrorBoundary } from '../ViewErrorBoundary';
import { SubViewTabs, subViewPanelId, subViewTabId, type SubViewTab } from '../SubViewTabs';
import { PreemptionWorklist } from './PreemptionWorklist';

// Supporting views load on first open, so the worklist's first paint never
// waits on the Blind Spots or Knowledge Gaps bundles (or their fetches).
const BlindSpotsView = lazy(() => import('../blindspots/BlindSpotsView'));
const KnowledgeGapsPanel = lazy(() => import('../KnowledgeGapsPanel').then(m => ({ default: m.KnowledgeGapsPanel })));

const ID_PREFIX = 'preemption';

/** Sub-views in display order. `signal`: Signal-tier (labelled for free users;
 *  the gate itself stays where it always was — backend + ProGate/paywall). */
const SUB_VIEWS: ReadonlyArray<{ id: PreemptionSubView; labelKey: string; signal: boolean }> = [
  { id: 'worklist', labelKey: 'preemption.views.worklist', signal: false },
  { id: 'blindspots', labelKey: 'preemption.views.blindspots', signal: true },
  { id: 'knowledge', labelKey: 'knowledgeGaps.title', signal: true },
];

/**
 * The Preemption tab (AD-054): the worklist — version-confirmed findings and
 * fix paths, plus the Upgrade Plan on Signal — with Blind Spots and Knowledge
 * Gaps folded in as supporting sub-views.
 */
const PreemptionView = memo(function PreemptionView() {
  const { t } = useTranslation();
  const { isPro } = useLicense();
  const { subView, setSubView } = useAppStore(
    useShallow(s => ({ subView: s.preemptionSubView, setSubView: s.setPreemptionSubView })),
  );

  const tabs = useMemo<SubViewTab<PreemptionSubView>[]>(
    () => SUB_VIEWS.map(({ id, labelKey, signal }) => ({
      id,
      label: t(labelKey),
      marker: signal && !isPro
        ? <span className="text-[10px] font-medium text-accent-gold" data-testid={`preemption-signal-marker-${id}`}>{t('tier.signal')}</span>
        : undefined,
    })),
    [t, isPro],
  );

  const select = useCallback((id: PreemptionSubView) => {
    if (id !== subView) trackEvent(`preemption_subview_open:${id}`, id);
    setSubView(id);
  }, [subView, setSubView]);

  return (
    <div className="space-y-5" role="tabpanel" id="view-panel-preemption" aria-labelledby="tab-preemption">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          {/* h2, not h1: App.tsx owns the document h1; sections below are h3. */}
          <h2 className="text-xl font-semibold text-text-primary tracking-tight">{t('preemption.title')}</h2>
          <p className="text-sm text-text-muted mt-1">{t('preemption.subtitle')}</p>
        </div>
        <SubViewTabs
          label={t('preemption.views.label')}
          idPrefix={ID_PREFIX}
          tabs={tabs}
          selected={subView}
          onSelect={select}
        />
      </header>

      <div
        role="tabpanel"
        id={subViewPanelId(ID_PREFIX, subView)}
        aria-labelledby={subViewTabId(ID_PREFIX, subView)}
      >
        {/* resetKey: switching sub-views clears a captured error, so a crash in
            one never strands the user — the sub-nav above stays clickable. */}
        <ViewErrorBoundary viewName={tabs.find(tab => tab.id === subView)?.label ?? t('nav.preemption.label')} resetKey={subView}>
          {subView === 'worklist' ? (
            <PreemptionWorklist />
          ) : (
            <Suspense fallback={<div className="flex items-center justify-center py-20 text-text-secondary text-sm">{t('action.loading')}</div>}>
              {subView === 'blindspots' ? <BlindSpotsView /> : <KnowledgeGapsPanel />}
            </Suspense>
          )}
        </ViewErrorBoundary>
      </div>
    </div>
  );
});

export default PreemptionView;
