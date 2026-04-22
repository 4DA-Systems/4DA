// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { trackEvent } from '../hooks/use-telemetry';
import type { ActiveView } from '../store/types';

const TABS: Array<{ id: ActiveView; labelKey: string; subtitleKey: string; activeColor: string }> = [
  { id: 'briefing', labelKey: 'nav.briefing.label', subtitleKey: 'nav.briefing.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'preemption', labelKey: 'nav.preemption.label', subtitleKey: 'nav.preemption.subtitle', activeColor: 'bg-red-500/20 text-red-400' },
  { id: 'blindspots', labelKey: 'nav.blindspots.label', subtitleKey: 'nav.blindspots.subtitle', activeColor: 'bg-amber-500/20 text-amber-400' },
  { id: 'results', labelKey: 'nav.signal.label', subtitleKey: 'nav.signal.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'playbook', labelKey: 'nav.playbook', subtitleKey: 'nav.playbook.subtitle', activeColor: 'bg-yellow-500/20 text-yellow-400' },
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

  return (
    <nav aria-label="Main views">
    <div className="mb-4 flex items-center gap-1 bg-bg-secondary rounded-lg p-1 border border-border w-fit" role="tablist" aria-label="Content views">
      {TABS.map(tab => {
        const showBadge = badges[tab.id] && activeView !== tab.id;
        return (
          <button
            key={tab.id}
            role="tab"
            aria-selected={activeView === tab.id}
            aria-controls={`view-panel-${tab.id}`}
            onClick={() => { trackEvent(`view_open:${tab.id}`, tab.id); setActiveView(tab.id); }}
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
                className={`absolute top-1 end-1 w-1.5 h-1.5 rounded-full ${BADGE_COLORS[tab.id] || 'bg-white/60'}`}
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
