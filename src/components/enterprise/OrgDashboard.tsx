import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

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

export function OrgDashboard() {
  const { t } = useTranslation();
  const organization = useAppStore(s => s.organization);
  const orgTeams = useAppStore(s => s.orgTeams);
  const orgAnalytics = useAppStore(s => s.orgAnalytics);
  const crossTeamSignals = useAppStore(s => s.crossTeamSignals);
  const orgLoading = useAppStore(s => s.orgLoading);
  const loadOrganization = useAppStore(s => s.loadOrganization);
  const loadOrgTeams = useAppStore(s => s.loadOrgTeams);
  const loadOrgAnalytics = useAppStore(s => s.loadOrgAnalytics);
  const loadCrossTeamSignals = useAppStore(s => s.loadCrossTeamSignals);

  useEffect(() => {
    loadOrganization();
    loadOrgTeams();
    loadOrgAnalytics();
    loadCrossTeamSignals();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (orgLoading && !organization) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="animate-pulse space-y-3">
          <div className="h-4 bg-border rounded w-1/3" />
          <div className="h-20 bg-border rounded" />
        </div>
      </div>
    );
  }

  if (!organization) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <h3 className="text-sm font-medium text-white mb-2">
          {t('enterprise.org.title', 'Organization')}
        </h3>
        <p className="text-xs text-text-muted">
          {t('enterprise.org.notConfigured', 'No organization configured. Organization management is available on the Enterprise tier.')}
        </p>
      </div>
    );
  }

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-medium text-white">
            {organization.name}
          </h3>
          <span className="text-[10px] px-1.5 py-0.5 bg-success/15 text-success rounded font-medium">
            {t('enterprise.org.enterprise', 'Enterprise')}
          </span>
        </div>
        <button
          onClick={() => { loadOrganization(); loadOrgTeams(); loadOrgAnalytics(); loadCrossTeamSignals(); }}
          className="text-[10px] text-text-muted hover:text-text-secondary transition-colors"
          aria-label="Refresh organization data"
        >
          {t('action.refresh', 'Refresh')}
        </button>
      </div>

      {/* Stats Row */}
      {orgAnalytics && (
        <div className="grid grid-cols-4 gap-3">
          {[
            { label: t('enterprise.org.teams', 'Teams'), value: orgAnalytics.total_seats, color: 'text-success' },
            { label: t('enterprise.org.activeSeats', 'Active Seats'), value: orgAnalytics.active_seats, color: 'text-white' },
            { label: t('enterprise.org.signalsMonth', 'Signals/Mo'), value: orgAnalytics.signals_detected, color: 'text-accent-gold' },
            { label: t('enterprise.org.decisionsMonth', 'Decisions/Mo'), value: orgAnalytics.decisions_tracked, color: 'text-[#818CF8]' },
          ].map(stat => (
            <div key={stat.label} className="bg-bg-primary rounded-lg p-3 border border-border/50">
              <p className="text-[10px] text-text-muted">{stat.label}</p>
              <p className={`text-lg font-semibold ${stat.color}`}>{stat.value}</p>
            </div>
          ))}
        </div>
      )}

      {/* Team List */}
      <div>
        <h4 className="text-xs font-medium text-text-secondary mb-2">
          {t('enterprise.org.teamList', 'Teams')} ({orgTeams.length})
        </h4>
        {orgTeams.length === 0 ? (
          <p className="text-xs text-text-muted">{t('enterprise.org.noTeams', 'No teams in this organization.')}</p>
        ) : (
          <div className="space-y-1.5">
            {orgTeams.map(team => (
              <div
                key={team.team_id}
                className="flex items-center justify-between px-3 py-2 bg-bg-primary rounded-lg border border-border/50"
              >
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-success" />
                  <span className="text-xs text-white font-mono">
                    {team.team_id.slice(0, 8)}...
                  </span>
                </div>
                <div className="flex items-center gap-3">
                  <span className="text-[10px] text-text-muted">
                    {team.member_count} {t('enterprise.org.members', 'members')}
                  </span>
                  {team.last_active && (
                    <span className="text-[10px] text-text-muted" title={team.last_active}>
                      {formatRelativeTime(team.last_active)}
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Cross-Team Signal Correlations */}
      {crossTeamSignals.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-secondary mb-2">
            {t('enterprise.org.crossTeamSignals', 'Cross-Team Signal Correlations')}
          </h4>
          <div className="space-y-1.5">
            {crossTeamSignals.slice(0, 5).map(sig => (
              <div
                key={sig.correlation_id}
                className="px-3 py-2.5 bg-bg-primary rounded-lg border border-border/50"
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-xs text-white font-medium">{sig.signal_type}</span>
                  <span className={`text-[10px] px-1.5 py-0.5 rounded ${
                    sig.org_severity === 'critical' ? 'bg-error/15 text-error' :
                    sig.org_severity === 'high' ? 'bg-[#F97316]/15 text-[#F97316]' :
                    'bg-accent-gold/15 text-accent-gold'
                  }`}>
                    {sig.org_severity}
                  </span>
                </div>
                <p className="text-[10px] text-text-muted">
                  {t('enterprise.org.affectsTeams', 'Affects')} {sig.teams_affected.length} {t('enterprise.org.teams', 'teams')}
                </p>
                <p className="text-[10px] text-text-secondary mt-1">{sig.recommendation}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Team Activities (from analytics) */}
      {orgAnalytics?.team_activity && orgAnalytics.team_activity.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-text-secondary mb-2">
            {t('enterprise.org.activity', 'Team Activity')}
          </h4>
          <div className="space-y-1">
            {orgAnalytics.team_activity.map(activity => (
              <div
                key={activity.team_id}
                className="flex items-center gap-3 px-3 py-1.5 text-[10px]"
              >
                <span className="text-text-muted font-mono w-20 truncate">{activity.team_id.slice(0, 8)}</span>
                <span className="text-text-secondary">{activity.active_members} members</span>
                <span className="text-accent-gold">{activity.signals_this_period} signals</span>
                <span className="text-[#818CF8]">{activity.decisions_this_period} decisions</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
