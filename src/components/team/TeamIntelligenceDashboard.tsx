import { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

type IntelTab = 'stack' | 'blindSpots' | 'overlap' | 'risk';

const INTEL_TABS: { key: IntelTab; labelKey: string; fallback: string }[] = [
  { key: 'stack', labelKey: 'team.intelligence.tabs.stack', fallback: 'Stack' },
  { key: 'blindSpots', labelKey: 'team.intelligence.tabs.blindSpots', fallback: 'Blind Spots' },
  { key: 'overlap', labelKey: 'team.intelligence.tabs.overlap', fallback: 'Overlap' },
  { key: 'risk', labelKey: 'team.intelligence.tabs.risk', fallback: 'Risk' },
];

export function TeamIntelligenceDashboard() {
  const { t } = useTranslation();
  const teamProfile = useAppStore(s => s.teamProfile);
  const teamProfileLoading = useAppStore(s => s.teamProfileLoading);
  const loadTeamProfile = useAppStore(s => s.loadTeamProfile);

  const [activeTab, setActiveTab] = useState<IntelTab>('stack');

  useEffect(() => {
    loadTeamProfile();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (teamProfileLoading && !teamProfile) {
    return (
      <div className="flex items-center justify-center py-6">
        <span className="text-xs text-text-muted">{t('action.loading', 'Loading...')}</span>
      </div>
    );
  }

  if (!teamProfile) {
    return (
      <div className="space-y-2">
        <p className="text-xs text-text-muted text-center py-6">
          {t('team.intelligence.empty', 'No team intelligence data yet. Members need to share their Developer DNA first.')}
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-white">
            {t('team.intelligence.title', 'Team Intelligence')}
          </span>
          <span className="text-[10px] text-text-muted">
            {teamProfile.member_count} {t('team.intelligence.memberCount', 'members')}
          </span>
        </div>
        {/* Coverage gauge */}
        <div className="flex items-center gap-1.5">
          <span className="text-[10px] text-text-muted">
            {t('team.intelligence.coverage', 'Stack Coverage')}
          </span>
          <div className="w-16 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              className="h-full bg-[#D4AF37] rounded-full transition-all"
              style={{ width: `${Math.round(teamProfile.stack_coverage * 100)}%` }}
            />
          </div>
          <span className="text-[10px] font-medium text-[#D4AF37]">
            {Math.round(teamProfile.stack_coverage * 100)}%
          </span>
        </div>
      </div>

      {/* Sub-tabs */}
      <div className="flex border-b border-border/30" role="tablist">
        {INTEL_TABS.map(tab => (
          <button
            key={tab.key}
            role="tab"
            aria-selected={activeTab === tab.key}
            onClick={() => setActiveTab(tab.key)}
            className={`flex-1 px-2 py-1.5 text-[10px] font-medium transition-all ${
              activeTab === tab.key
                ? 'text-[#D4AF37] border-b border-[#D4AF37]'
                : 'text-text-muted hover:text-text-secondary'
            }`}
          >
            {t(tab.labelKey, tab.fallback)}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      <div className="max-h-52 overflow-y-auto">
        {activeTab === 'stack' && <StackTab profile={teamProfile} />}
        {activeTab === 'blindSpots' && <BlindSpotsTab profile={teamProfile} />}
        {activeTab === 'overlap' && <OverlapTab profile={teamProfile} />}
        {activeTab === 'risk' && <RiskTab profile={teamProfile} />}
      </div>

      {/* Footer timestamp */}
      {teamProfile.generated_at && (
        <div className="text-[10px] text-text-muted text-right">
          {t('team.intelligence.generatedAt', 'Generated')}: {formatRelative(teamProfile.generated_at)}
        </div>
      )}
    </div>
  );
}

// ---- Sub-tab components ----

interface TabProps {
  profile: NonNullable<ReturnType<typeof useAppStore.getState>['teamProfile']>;
}

function StackTab({ profile }: TabProps) {
  const { t } = useTranslation();

  const sorted = useMemo(
    () => [...profile.collective_stack].sort((a, b) => b.team_confidence - a.team_confidence),
    [profile.collective_stack],
  );

  if (profile.collective_stack.length === 0) {
    return (
      <p className="text-xs text-text-muted text-center py-4">
        {t('team.intelligence.stack.empty', 'No collective stack data yet. Members need to share their Developer DNA first.')}
      </p>
    );
  }

  return (
    <div className="space-y-1.5">
      {sorted.map(entry => (
        <div key={entry.tech} className="px-2 py-1.5 rounded bg-bg-primary/50 border border-border/30">
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs text-white font-medium">{entry.tech}</span>
            <span className="text-[10px] px-1.5 py-0.5 bg-[#D4AF37]/10 text-[#D4AF37] rounded">
              {entry.members.length} {t('team.intelligence.stack.trackedBy', 'tracked by')}
            </span>
          </div>
          {/* Confidence bar */}
          <div className="flex items-center gap-1.5">
            <div className="flex-1 h-1 bg-bg-tertiary rounded-full overflow-hidden">
              <div
                className="h-full bg-[#D4AF37] rounded-full transition-all"
                style={{ width: `${Math.round(entry.team_confidence * 100)}%` }}
              />
            </div>
            <span className="text-[10px] text-text-muted">
              {Math.round(entry.team_confidence * 100)}% {t('team.intelligence.stack.confidence', 'confidence')}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

function BlindSpotsTab({ profile }: TabProps) {
  const { t } = useTranslation();

  if (profile.blind_spots.length === 0) {
    return (
      <p className="text-xs text-text-muted text-center py-4">
        {t('team.intelligence.blindSpots.empty', 'No blind spots detected. Your team has strong coverage.')}
      </p>
    );
  }

  const severityColor = (sev: string) => {
    switch (sev) {
      case 'high': return 'bg-[#EF4444]/15 text-[#EF4444]';
      case 'medium': return 'bg-[#D4AF37]/15 text-[#D4AF37]';
      default: return 'bg-border text-text-muted';
    }
  };

  return (
    <div className="space-y-1.5">
      {profile.blind_spots.map(spot => (
        <div key={spot.topic} className="px-2 py-1.5 rounded bg-bg-primary/50 border border-border/30">
          <div className="flex items-center justify-between mb-0.5">
            <span className="text-xs text-white font-medium">{spot.topic}</span>
            <span className={`text-[10px] px-1.5 py-0.5 rounded ${severityColor(spot.severity)}`}>
              {t(`team.intelligence.blindSpots.severity.${spot.severity}`, spot.severity)}
            </span>
          </div>
          {spot.related_to.length > 0 && (
            <div className="flex items-center gap-1 flex-wrap">
              <span className="text-[10px] text-text-muted">
                {t('team.intelligence.blindSpots.relatedTo', 'Related to')}:
              </span>
              {spot.related_to.map(tag => (
                <span key={tag} className="text-[10px] px-1 py-0.5 bg-bg-tertiary text-text-secondary rounded">
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

function OverlapTab({ profile }: TabProps) {
  const { t } = useTranslation();

  if (profile.overlap_zones.length === 0) {
    return (
      <p className="text-xs text-text-muted text-center py-4">
        {t('team.intelligence.overlap.empty', 'No overlap zones detected yet. More member data needed.')}
      </p>
    );
  }

  return (
    <div className="space-y-1.5">
      {profile.overlap_zones.map(zone => (
        <div key={zone.topic} className="px-2 py-1.5 rounded bg-bg-primary/50 border border-border/30">
          <div className="flex items-center justify-between mb-0.5">
            <span className="text-xs text-white font-medium">{zone.topic}</span>
            <span className="text-[10px] text-text-muted">
              {zone.member_count} {t('team.intelligence.overlap.members', 'members tracking')}
            </span>
          </div>
          <div className="flex items-center gap-1">
            {zone.members.slice(0, 5).map(name => (
              <span
                key={name}
                className="w-5 h-5 rounded-full bg-bg-tertiary border border-border/50 flex items-center justify-center text-[9px] text-text-secondary"
                title={name}
              >
                {name.charAt(0).toUpperCase()}
              </span>
            ))}
            {zone.members.length > 5 && (
              <span className="text-[10px] text-text-muted">+{zone.members.length - 5}</span>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

function RiskTab({ profile }: TabProps) {
  const { t } = useTranslation();

  if (profile.unique_strengths.length === 0) {
    return (
      <p className="text-xs text-text-muted text-center py-4">
        {t('team.intelligence.risk.empty', 'No single-expert risks detected. Good redundancy.')}
      </p>
    );
  }

  const riskColor = (level: string) => {
    switch (level) {
      case 'high': return 'bg-[#EF4444]/15 text-[#EF4444]';
      case 'medium': return 'bg-[#D4AF37]/15 text-[#D4AF37]';
      default: return 'bg-border text-text-muted';
    }
  };

  return (
    <div className="space-y-1.5">
      {profile.unique_strengths.map(strength => (
        <div key={strength.tech} className="px-2 py-1.5 rounded bg-bg-primary/50 border border-border/30">
          <div className="flex items-center justify-between mb-0.5">
            <span className="text-xs text-white font-medium">{strength.tech}</span>
            <span className={`text-[10px] px-1.5 py-0.5 rounded ${riskColor(strength.risk_level)}`}>
              {t(`team.intelligence.risk.${strength.risk_level}`, strength.risk_level)} {t('team.intelligence.risk.riskLevel', 'Risk')}
            </span>
          </div>
          <div className="text-[10px] text-text-muted">
            {t('team.intelligence.risk.soleExpert', 'Sole expert')}: <span className="text-text-secondary">{strength.sole_expert}</span>
          </div>
        </div>
      ))}
    </div>
  );
}

function formatRelative(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}
