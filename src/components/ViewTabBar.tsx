import { useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { trackEvent } from '../hooks/use-telemetry';
import type { ViewTier } from '../store/types';

type ViewId = 'briefing' | 'chapters' | 'results' | 'preemption' | 'blindspots' | 'profile' | 'insights' | 'saved' | 'toolkit' | 'playbook' | 'calibrate' | 'console';

const TABS: Array<{ id: ViewId; labelKey: string; subtitleKey: string; activeColor: string }> = [
  { id: 'briefing', labelKey: 'nav.briefing.label', subtitleKey: 'nav.briefing.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'preemption', labelKey: 'nav.preemption.label', subtitleKey: 'nav.preemption.subtitle', activeColor: 'bg-red-500/20 text-red-400' },
  { id: 'blindspots', labelKey: 'nav.blindspots.label', subtitleKey: 'nav.blindspots.subtitle', activeColor: 'bg-amber-500/20 text-amber-400' },
  { id: 'chapters', labelKey: 'nav.chapters', subtitleKey: 'nav.chapters.subtitle', activeColor: 'bg-indigo-500/20 text-indigo-400' },
  { id: 'results', labelKey: 'nav.results', subtitleKey: 'nav.results.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'playbook', labelKey: 'nav.playbook', subtitleKey: 'nav.playbook.subtitle', activeColor: 'bg-yellow-500/20 text-yellow-400' },
  { id: 'insights', labelKey: 'nav.insights', subtitleKey: 'nav.insights.subtitle', activeColor: 'bg-amber-500/20 text-amber-400' },
  { id: 'saved', labelKey: 'nav.saved', subtitleKey: 'nav.saved.subtitle', activeColor: 'bg-green-500/20 text-green-400' },
  { id: 'profile', labelKey: 'nav.profile', subtitleKey: 'nav.profile.subtitle', activeColor: 'bg-white/10 text-white' },
  { id: 'console', labelKey: 'nav.console', subtitleKey: 'nav.console.subtitle', activeColor: 'bg-emerald-500/20 text-emerald-400' },
  { id: 'toolkit', labelKey: 'nav.toolkit', subtitleKey: 'nav.toolkit.subtitle', activeColor: 'bg-purple-500/20 text-purple-400' },
  { id: 'calibrate', labelKey: 'nav.calibrate', subtitleKey: 'nav.calibrate.subtitle', activeColor: 'bg-sky-500/20 text-sky-400' },
];

// CANONICAL SOURCE: The list of views visible per tier.
// MUST stay in sync with TIER_VIEWS in src/store/ui-slice.ts.
// Exported for the consistency test at src/components/__tests__/tier-views-consistency.test.ts.
export const TIER_VIEWS: Record<ViewTier, ViewId[]> = {
  core: ['briefing', 'chapters', 'results', 'playbook'],
  explorer: ['briefing', 'preemption', 'blindspots', 'chapters', 'results', 'playbook', 'insights'],
  invested: ['briefing', 'preemption', 'blindspots', 'chapters', 'results', 'playbook', 'insights', 'saved', 'profile', 'console'],
  power: ['briefing', 'preemption', 'blindspots', 'chapters', 'results', 'playbook', 'insights', 'saved', 'profile', 'console', 'toolkit', 'calibrate'],
};

const BADGE_COLORS: Partial<Record<ViewId, string>> = {
  briefing: 'bg-orange-400',
  results: 'bg-orange-400',
  profile: 'bg-amber-400',
  insights: 'bg-amber-400',
  saved: 'bg-green-400',
};

export const ViewTabBar = memo(function ViewTabBar() {
  const { t } = useTranslation();
  const { activeView, resultsCount, windows, profilePct, viewTier, showAllViews, savedCount, wisdomCount } = useAppStore(
    useShallow((s) => ({
      activeView: s.activeView,
      resultsCount: s.appState.relevanceResults.length,
      windows: s.decisionWindows,
      profilePct: s.unifiedProfile?.completeness.overall_percentage,
      viewTier: s.viewTier,
      showAllViews: s.showAllViews,
      savedCount: Object.values(s.feedbackGiven).filter(f => f === 'save').length,
      wisdomCount: (s.decisionWindows ?? []).filter(w => w.status === 'open').length,
    })),
  );
  const setActiveView = useAppStore(s => s.setActiveView);

  const badges = useMemo(() => {
    const b: Partial<Record<ViewId, number | boolean>> = {};
    if (resultsCount > 0) b.results = true;
    if ((windows ?? []).some(w => w.status === 'open')) b.briefing = true;
    if (profilePct != null && profilePct < 50) b.profile = true;
    // New notification badges with counts
    if (wisdomCount > 0) b.insights = wisdomCount;
    if (savedCount > 0) b.saved = savedCount;
    return b;
  }, [resultsCount, windows, profilePct, wisdomCount, savedCount]);

  const visibleTabs = useMemo(() => {
    if (showAllViews) return TABS;
    const allowed = TIER_VIEWS[viewTier];
    return TABS.filter(tab => allowed.includes(tab.id));
  }, [viewTier, showAllViews]);

  return (
    <nav aria-label="Main views">
    <div className="mb-4 flex items-center gap-1 bg-bg-secondary rounded-lg p-1 border border-border w-fit" role="tablist" aria-label="Content views">
      {visibleTabs.map(tab => {
        const badgeValue = badges[tab.id];
        const showBadge = badgeValue && activeView !== tab.id;
        const badgeCount = typeof badgeValue === 'number' ? badgeValue : 0;
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
              badgeCount > 0 ? (
                <span
                  className={`absolute -top-1 -end-1 min-w-[18px] h-[18px] flex items-center justify-center rounded-full text-[10px] font-bold text-black ${BADGE_COLORS[tab.id] || 'bg-white/60'}`}
                  aria-label={`${badgeCount} notifications`}
                >
                  {badgeCount > 9 ? '9+' : badgeCount}
                </span>
              ) : (
                <span
                  className={`absolute top-1 end-1 w-1.5 h-1.5 rounded-full ${BADGE_COLORS[tab.id] || 'bg-white/60'}`}
                  aria-label="New activity"
                />
              )
            )}
          </button>
        );
      })}
    </div>
    </nav>
  );
});
