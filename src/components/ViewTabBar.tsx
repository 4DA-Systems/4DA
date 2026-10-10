// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useMemo, memo, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { trackEvent } from '../hooks/use-telemetry';
import type { ActiveView } from '../store/types';

// Main nav is three tabs (AD-054; doctrine rule 2). Blind Spots and Knowledge
// Gaps are Preemption sub-views (PreemptionView's own sub-navigation).
const TABS: Array<{ id: ActiveView; labelKey: string; subtitleKey: string; activeColor: string }> = [
  { id: 'briefing', labelKey: 'nav.briefing.label', subtitleKey: 'nav.briefing.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'preemption', labelKey: 'nav.preemption.label', subtitleKey: 'nav.preemption.subtitle', activeColor: 'bg-red-500/20 text-red-400' },
  { id: 'results', labelKey: 'nav.signal.label', subtitleKey: 'nav.signal.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
];

const BADGE_COLORS: Partial<Record<ActiveView, string>> = {
  briefing: 'bg-orange-400',
  results: 'bg-orange-400',
};

export const ViewTabBar = memo(function ViewTabBar() {
  const { t } = useTranslation();
  const { activeView, resultsCount, windows } = useAppStore(
    useShallow((s) => ({
      activeView: s.activeView,
      resultsCount: s.appState.relevanceResults.length,
      windows: s.decisionWindows,
    })),
  );
  const setActiveView = useAppStore(s => s.setActiveView);

  const badges = useMemo(() => {
    const b: Partial<Record<ActiveView, boolean>> = {};
    if (resultsCount > 0) b.results = true;
    if ((windows ?? []).some(w => w.status === 'open')) b.briefing = true;
    return b;
  }, [resultsCount, windows]);

  const activate = (id: ActiveView) => {
    trackEvent(`view_open:${id}`, id);
    setActiveView(id);
  };

  // WAI-ARIA tabs, roving tabindex: only the selected tab (or the first, when
  // none is) is in the Tab order; arrows / Home / End move focus and activate.
  const selectedIndex = TABS.findIndex((tab) => tab.id === activeView);
  const focusableIndex = selectedIndex >= 0 ? selectedIndex : 0;

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>, current: number) => {
    const list = e.currentTarget.closest('[role="tablist"]');
    if (!list) return;
    const rtl = window.getComputedStyle(list).direction === 'rtl';
    const forward = rtl ? 'ArrowLeft' : 'ArrowRight';
    const backward = rtl ? 'ArrowRight' : 'ArrowLeft';
    let next: number;
    if (e.key === forward) next = (current + 1) % TABS.length;
    else if (e.key === backward) next = (current - 1 + TABS.length) % TABS.length;
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = TABS.length - 1;
    else return;
    e.preventDefault();
    const tab = TABS[next]!;
    list.querySelector<HTMLButtonElement>(`#tab-${tab.id}`)?.focus();
    activate(tab.id);
  };

  return (
    <nav aria-label="Main views">
    <div
      className="mb-4 flex items-center gap-1 bg-bg-secondary rounded-lg p-1 border border-border w-fit"
      role="tablist"
      aria-label="Content views"
      aria-orientation="horizontal"
    >
      {TABS.map((tab, index) => {
        const showBadge = badges[tab.id] && activeView !== tab.id;
        return (
          <button
            key={tab.id}
            id={`tab-${tab.id}`}
            role="tab"
            aria-selected={activeView === tab.id}
            aria-controls={`view-panel-${tab.id}`}
            tabIndex={index === focusableIndex ? 0 : -1}
            onClick={() => activate(tab.id)}
            onKeyDown={(e) => onKeyDown(e, index)}
            className={`relative px-3 py-1.5 text-sm rounded-md transition-all ${
              activeView === tab.id
                ? `${tab.activeColor} font-medium`
                : 'text-text-muted hover:text-text-secondary'
            }`}
            title={t(tab.subtitleKey)}
          >
            <span>{t(tab.labelKey)}</span>
            {showBadge && (
              <span
                className={`absolute top-1 end-1 w-1.5 h-1.5 rounded-full ${BADGE_COLORS[tab.id] || 'bg-text-primary/60'}`}
                aria-label="New activity"
              />
            )}
          </button>
        );
      })}
    </div>
    </nav>
  );
});
