import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

type ViewId = 'briefing' | 'channels' | 'results' | 'profile' | 'insights' | 'saved' | 'toolkit' | 'playbook' | 'coach' | 'calibrate';

const TABS: Array<{ id: ViewId; labelKey: string; subtitleKey: string; activeColor: string }> = [
  { id: 'briefing', labelKey: 'nav.briefing.label', subtitleKey: 'nav.briefing.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'channels', labelKey: 'nav.channels', subtitleKey: 'nav.channels.subtitle', activeColor: 'bg-cyan-500/20 text-cyan-400' },
  { id: 'results', labelKey: 'nav.results', subtitleKey: 'nav.results.subtitle', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'profile', labelKey: 'nav.profile', subtitleKey: 'nav.profile.subtitle', activeColor: 'bg-white/10 text-white' },
  { id: 'insights', labelKey: 'nav.insights', subtitleKey: 'nav.insights.subtitle', activeColor: 'bg-amber-500/20 text-amber-400' },
  { id: 'saved', labelKey: 'nav.saved', subtitleKey: 'nav.saved.subtitle', activeColor: 'bg-green-500/20 text-green-400' },
  { id: 'toolkit', labelKey: 'nav.toolkit', subtitleKey: 'nav.toolkit.subtitle', activeColor: 'bg-purple-500/20 text-purple-400' },
  { id: 'playbook', labelKey: 'nav.playbook', subtitleKey: 'nav.playbook.subtitle', activeColor: 'bg-yellow-500/20 text-yellow-400' },
  { id: 'coach', labelKey: 'nav.coach', subtitleKey: 'nav.coach.subtitle', activeColor: 'bg-emerald-500/20 text-emerald-400' },
  { id: 'calibrate', labelKey: 'nav.calibrate', subtitleKey: 'nav.calibrate.subtitle', activeColor: 'bg-sky-500/20 text-sky-400' },
];

const BADGE_COLORS: Partial<Record<ViewId, string>> = {
  briefing: 'bg-orange-400',
  channels: 'bg-cyan-400',
  results: 'bg-orange-400',
  profile: 'bg-amber-400',
};

export function ViewTabBar() {
  const { t } = useTranslation();
  const activeView = useAppStore(s => s.activeView);
  const setActiveView = useAppStore(s => s.setActiveView);
  const results = useAppStore(s => s.appState.relevanceResults);
  const windows = useAppStore(s => s.decisionWindows);
  const profile = useAppStore(s => s.unifiedProfile);
  const channels = useAppStore(s => s.channels) ?? [];

  const badges = useMemo(() => {
    const b: Partial<Record<ViewId, boolean>> = {};
    if (results.length > 0) b.results = true;
    if ((windows ?? []).some(w => w.status === 'open')) b.briefing = true;
    if (profile && profile.completeness.overall_percentage < 50) b.profile = true;
    if (channels.some(ch => ch.freshness === 'fresh')) b.channels = true;
    return b;
  }, [results, windows, profile, channels]);

  return (
    <nav aria-label="Main views">
    <div className="mb-6 flex items-center gap-1 bg-bg-secondary rounded-lg p-1 border border-border w-fit" role="tablist" aria-label="Content views">
      {TABS.map(tab => {
        const showBadge = badges[tab.id] && activeView !== tab.id;
        return (
          <button
            key={tab.id}
            role="tab"
            aria-selected={activeView === tab.id}
            aria-controls={`view-panel-${tab.id}`}
            onClick={() => setActiveView(tab.id)}
            className={`relative px-4 py-1.5 text-sm rounded-md transition-all ${
              activeView === tab.id
                ? `${tab.activeColor} font-medium`
                : 'text-gray-500 hover:text-gray-300'
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
}
