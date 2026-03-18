import { useMemo, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { useAppStore } from '../store';
import { trackEvent } from '../hooks/use-telemetry';
import type { ViewTier } from '../store/types';

type ViewId = 'briefing' | 'channels' | 'results' | 'profile' | 'insights' | 'saved' | 'toolkit' | 'playbook' | 'calibrate' | 'console';

const TABS: Array<{ id: ViewId; labelKey: string; subtitleKey: string; activeColor: string }> = [
  { id: 'briefing', labelKey: 'nav.briefing.label', subtitleKey: 'nav.briefing.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'results', labelKey: 'nav.results', subtitleKey: 'nav.results.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'playbook', labelKey: 'nav.playbook', subtitleKey: 'nav.playbook.subtitle', activeColor: 'bg-yellow-500/20 text-yellow-400' },
  { id: 'channels', labelKey: 'nav.channels', subtitleKey: 'nav.channels.subtitle', activeColor: 'bg-cyan-500/20 text-cyan-400' },
  { id: 'insights', labelKey: 'nav.insights', subtitleKey: 'nav.insights.subtitle', activeColor: 'bg-amber-500/20 text-amber-400' },
  { id: 'saved', labelKey: 'nav.saved', subtitleKey: 'nav.saved.subtitle', activeColor: 'bg-green-500/20 text-green-400' },
  { id: 'profile', labelKey: 'nav.profile', subtitleKey: 'nav.profile.subtitle', activeColor: 'bg-white/10 text-white' },
  { id: 'console', labelKey: 'nav.console', subtitleKey: 'nav.console.subtitle', activeColor: 'bg-emerald-500/20 text-emerald-400' },
  { id: 'toolkit', labelKey: 'nav.toolkit', subtitleKey: 'nav.toolkit.subtitle', activeColor: 'bg-purple-500/20 text-purple-400' },
  { id: 'calibrate', labelKey: 'nav.calibrate', subtitleKey: 'nav.calibrate.subtitle', activeColor: 'bg-sky-500/20 text-sky-400' },
];

const TIER_VIEWS: Record<ViewTier, ViewId[]> = {
  core: ['briefing', 'results', 'playbook'],
  explorer: ['briefing', 'results', 'playbook', 'channels', 'insights'],
  invested: ['briefing', 'results', 'playbook', 'channels', 'insights', 'saved', 'profile', 'console'],
  power: ['briefing', 'results', 'playbook', 'channels', 'insights', 'saved', 'profile', 'console', 'toolkit', 'calibrate'],
};

const BADGE_COLORS: Partial<Record<ViewId, string>> = {
  briefing: 'bg-orange-400',
  channels: 'bg-cyan-400',
  results: 'bg-orange-400',
  profile: 'bg-amber-400',
};

export const ViewTabBar = memo(function ViewTabBar() {
  const { t } = useTranslation();
  const { activeView, resultsCount, windows, profilePct, channels, viewTier, showAllViews } = useAppStore(
    useShallow((s) => ({
      activeView: s.activeView,
      resultsCount: s.appState.relevanceResults.length,
      windows: s.decisionWindows,
      profilePct: s.unifiedProfile?.completeness.overall_percentage,
      channels: s.channels ?? [],
      viewTier: s.viewTier,
      showAllViews: s.showAllViews,
    })),
  );
  const setActiveView = useAppStore(s => s.setActiveView);

  const badges = useMemo(() => {
    const b: Partial<Record<ViewId, boolean>> = {};
    if (resultsCount > 0) b.results = true;
    if ((windows ?? []).some(w => w.status === 'open')) b.briefing = true;
    if (profilePct != null && profilePct < 50) b.profile = true;
    if (channels.some(ch => ch.freshness === 'fresh')) b.channels = true;
    return b;
  }, [resultsCount, windows, profilePct, channels]);

  const visibleTabs = useMemo(() => {
    if (showAllViews) return TABS;
    const allowed = TIER_VIEWS[viewTier];
    return TABS.filter(tab => allowed.includes(tab.id));
  }, [viewTier, showAllViews]);

  return (
    <nav aria-label="Main views">
    <div className="mb-6 flex items-center gap-1 bg-bg-secondary rounded-lg p-1 border border-border w-fit" role="tablist" aria-label="Content views">
      {visibleTabs.map(tab => {
        const showBadge = badges[tab.id] && activeView !== tab.id;
        return (
          <button
            key={tab.id}
            role="tab"
            aria-selected={activeView === tab.id}
            aria-controls={`view-panel-${tab.id}`}
            onClick={() => { trackEvent(`view_open:${tab.id}`, tab.id); setActiveView(tab.id); }}
            className={`relative px-4 py-1.5 text-sm rounded-md transition-all ${
              activeView === tab.id
                ? `${tab.activeColor} font-medium`
                : 'text-text-muted hover:text-text-secondary'
            }`}
            title={t(tab.subtitleKey)}
          >
            <span>{t(tab.labelKey)}</span>
            <span className={`block text-[10px] leading-tight ${
              activeView === tab.id ? 'opacity-70' : 'opacity-40'
            }`}>{t(tab.subtitleKey)}</span>
            {showBadge && (
              <span
                className={`absolute top-1 right-1 w-1.5 h-1.5 rounded-full ${BADGE_COLORS[tab.id] || 'bg-white/60'}`}
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
