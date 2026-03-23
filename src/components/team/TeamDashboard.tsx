import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useGameComponent, type GameElement } from '../../hooks/use-game-component';
import { TeamMemberList } from './TeamMemberList';
import { TeamSignalFeed } from './TeamSignalFeed';
import { TeamDecisionTracker } from './TeamDecisionTracker';
import { TeamSharedSources } from './TeamSharedSources';
import { TeamIntelligenceDashboard } from './TeamIntelligenceDashboard';

type DashboardTab = 'intelligence' | 'signals' | 'decisions' | 'sources' | 'members';

const TABS: { key: DashboardTab; labelKey: string; fallback: string }[] = [
  { key: 'intelligence', labelKey: 'team.dashboard.intelligence', fallback: 'Intelligence' },
  { key: 'signals', labelKey: 'team.dashboard.signals', fallback: 'Signals' },
  { key: 'decisions', labelKey: 'team.dashboard.decisions', fallback: 'Decisions' },
  { key: 'sources', labelKey: 'team.dashboard.sources', fallback: 'Sources' },
  { key: 'members', labelKey: 'team.dashboard.memberList', fallback: 'Members' },
];

export function TeamDashboard() {
  const { t } = useTranslation();
  const teamStatus = useAppStore(s => s.teamStatus);
  const teamLoading = useAppStore(s => s.teamLoading);
  const loadTeamStatus = useAppStore(s => s.loadTeamStatus);
  const loadTeamMembers = useAppStore(s => s.loadTeamMembers);
  const tier = useAppStore(s => s.tier);

  const [collapsed, setCollapsed] = useState(false);
  const [activeTab, setActiveTab] = useState<DashboardTab>('intelligence');
  const { containerRef: icoContainerRef, elementRef: icoElementRef } = useGameComponent('game-icosahedron');

  useEffect(() => {
    if (tier === 'team' || tier === 'enterprise') {
      loadTeamStatus();
      loadTeamMembers();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tier]);

  // Sync icosahedron pulse to relay activity
  const setIcoParam = useCallback((name: string, value: number) => {
    (icoElementRef.current as GameElement)?.setParam?.(name, value);
  }, [icoElementRef]);

  useEffect(() => {
    const pending = teamStatus?.pending_outbound ?? 0;
    setIcoParam('pulse', pending > 0 ? 1.0 : 0.0);
    setIcoParam('glow_intensity', teamStatus?.connected ? 1.0 : 0.4);
  }, [teamStatus, setIcoParam]);

  // Don't render if not on a team tier or no team configured
  if (tier !== 'team' && tier !== 'enterprise') return null;
  if (!teamStatus?.enabled) return null;

  const isConnected = teamStatus.connected;

  return (
    <div className="bg-bg-secondary border border-border rounded-xl overflow-hidden">
      {/* Header Bar */}
      <button
        onClick={() => setCollapsed(prev => !prev)}
        className="w-full px-4 py-3 flex items-center justify-between hover:bg-bg-tertiary/50 transition-colors"
        aria-expanded={!collapsed}
        aria-controls="team-dashboard-content"
      >
        <div className="flex items-center gap-2">
          {/* Icosahedral network visualization */}
          <div
            ref={icoContainerRef}
            className="w-6 h-6 rounded overflow-hidden opacity-80"
          />
          {/* Connection indicator */}
          <div className={`w-2 h-2 rounded-full ${
            isConnected ? 'bg-[#22C55E]' : 'bg-[#EF4444]'
          }`} />
          <span className="text-xs font-medium text-white">
            {t('team.dashboard.title', 'Team')}
          </span>
          <span className="text-[10px] text-text-muted">
            {teamStatus.member_count} {t('team.dashboard.members', 'members')}
          </span>
          {teamStatus.pending_outbound > 0 && (
            <span className="text-[10px] px-1.5 py-0.5 bg-[#D4AF37]/15 text-[#D4AF37] rounded">
              {teamStatus.pending_outbound} {t('team.dashboard.pending', 'pending')}
            </span>
          )}
        </div>
        <span className={`text-text-muted text-xs transition-transform ${collapsed ? '' : 'rotate-180'}`}>
          &#9660;
        </span>
      </button>

      {/* Content */}
      {!collapsed && (
        <div id="team-dashboard-content" className="border-t border-border">
          {/* Tab Bar */}
          <div className="flex border-b border-border/50" role="tablist">
            {TABS.map(tab => (
              <button
                key={tab.key}
                id={`team-tab-${tab.key}`}
                role="tab"
                aria-selected={activeTab === tab.key}
                aria-controls={`team-tabpanel-${tab.key}`}
                onClick={() => setActiveTab(tab.key)}
                className={`flex-1 px-3 py-2 text-[10px] font-medium transition-all ${
                  activeTab === tab.key
                    ? 'text-[#22C55E] border-b border-[#22C55E]'
                    : 'text-text-muted hover:text-text-secondary'
                }`}
              >
                {t(tab.labelKey, tab.fallback)}
              </button>
            ))}
          </div>

          {/* Tab Content */}
          <div id={`team-tabpanel-${activeTab}`} role="tabpanel" aria-labelledby={`team-tab-${activeTab}`} className="p-3 max-h-80 overflow-y-auto">
            {teamLoading ? (
              <div className="flex items-center justify-center py-6">
                <span className="text-xs text-text-muted">{t('action.loading', 'Loading...')}</span>
              </div>
            ) : activeTab === 'intelligence' ? (
              <TeamIntelligenceDashboard />
            ) : activeTab === 'signals' ? (
              <TeamSignalFeed />
            ) : activeTab === 'decisions' ? (
              <TeamDecisionTracker />
            ) : activeTab === 'sources' ? (
              <TeamSharedSources />
            ) : (
              <TeamMemberList />
            )}
          </div>

          {/* Footer */}
          {teamStatus.last_sync_at && (
            <div className="px-3 py-1.5 border-t border-border/50 flex items-center justify-between">
              <span className="text-[10px] text-text-muted">
                {t('team.dashboard.lastSync', 'Last sync')}: {formatRelativeTime(teamStatus.last_sync_at)}
              </span>
              <button
                onClick={() => { loadTeamStatus(); loadTeamMembers(); }}
                className="text-[10px] text-text-muted hover:text-[#22C55E] transition-colors"
                aria-label="Refresh team data"
              >
                {t('action.refresh', 'Refresh')}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}
