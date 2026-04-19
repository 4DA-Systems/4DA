// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { lazy, Suspense, useEffect, useState, memo, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import { cmd } from '../lib/commands';
import type { DimensionCompleteness } from '../types/profile';
import { DeveloperDnaSection } from './DeveloperDnaSection';

const LearningIndicator = lazy(() => import('./LearningIndicator').then(m => ({ default: m.LearningIndicator })));

// ============================================================================
// Completeness Ring (SVG)
// ============================================================================

function CompletenessRing({ percentage, size = 48 }: { percentage: number; size?: number }) {
  const radius = (size - 6) / 2;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference - (percentage / 100) * circumference;

  return (
    <svg width={size} height={size} className="transform -rotate-90" role="img" aria-label={`${Math.round(percentage)}% complete`}>
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke="currentColor"
        strokeWidth={3}
        className="text-white/10"
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke="currentColor"
        strokeWidth={3}
        strokeDasharray={circumference}
        strokeDashoffset={offset}
        strokeLinecap="round"
        className={percentage >= 70 ? 'text-green-500' : percentage >= 40 ? 'text-amber-500' : 'text-red-400'}
      />
    </svg>
  );
}

// ============================================================================
// Depth Badge
// ============================================================================

function DepthBadge({ depth }: { depth: string }) {
  const colors: Record<string, string> = {
    empty: 'bg-white/5 text-text-muted',
    minimal: 'bg-red-500/10 text-red-400',
    partial: 'bg-amber-500/10 text-amber-400',
    good: 'bg-green-500/10 text-green-400',
    comprehensive: 'bg-emerald-500/10 text-emerald-300',
  };
  return (
    <span className={`px-2 py-0.5 text-[10px] rounded-full font-medium ${colors[depth] || colors.empty}`}>
      {depth}
    </span>
  );
}

// ============================================================================
// Dimension Card
// ============================================================================

// Actionable suggestions per dimension when data is thin — with CTA buttons
const DIMENSION_ACTIONS: Record<string, { labelKey: string; buttonLabelKey: string; action: string }> = {
  Infrastructure: { labelKey: 'sovereignProfile.action.infraLabel', buttonLabelKey: 'sovereignProfile.action.scanNow', action: 'scan_infra' },
  Stack: { labelKey: 'sovereignProfile.action.stackLabel', buttonLabelKey: 'sovereignProfile.action.scanProjects', action: 'scan_stack' },
  Skills: { labelKey: 'sovereignProfile.action.skillsLabel', buttonLabelKey: 'sovereignProfile.action.openPlaybook', action: 'open_playbook' },
  Preferences: { labelKey: 'sovereignProfile.action.prefsLabel', buttonLabelKey: 'sovereignProfile.action.openSettings', action: 'open_settings' },
  Context: { labelKey: 'sovereignProfile.action.contextLabel', buttonLabelKey: 'sovereignProfile.action.openSettings', action: 'open_settings' },
};

function DimensionCard({ dim, children, onAction }: { dim: DimensionCompleteness; children: React.ReactNode; onAction?: (action: string) => void }) {
  const { t } = useTranslation();
  const needsAction = dim.depth === 'empty' || dim.depth === 'minimal';
  const actionDef = needsAction ? DIMENSION_ACTIONS[dim.name] : null;

  return (
    <div className="bg-bg-secondary border border-border rounded-lg p-4">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-3">
          <CompletenessRing percentage={dim.percentage} size={36} />
          <div>
            <h3 className="text-sm font-medium text-white">{dim.name}</h3>
            <span className="text-[10px] text-text-muted">{t('profile.factCount', { count: dim.fact_count })}</span>
          </div>
        </div>
        <DepthBadge depth={dim.depth} />
      </div>
      <div className="text-xs text-text-secondary space-y-1">{children}</div>
      {actionDef && (
        <div className="mt-2 flex items-center gap-2">
          <span className="text-[10px] text-amber-400/80">{t(actionDef.labelKey)}</span>
          <button
            onClick={() => onAction?.(actionDef.action)}
            aria-label={t(actionDef.buttonLabelKey)}
            className="px-2 py-0.5 text-[10px] font-medium text-black bg-amber-400 hover:bg-amber-300 rounded transition-colors"
          >
            {t(actionDef.buttonLabelKey)}
          </button>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Main Component
// ============================================================================

const EMPTY_DIM = { depth: 'empty' as const, fact_count: 0, percentage: 0 };

export const SovereignDeveloperProfile = memo(function SovereignDeveloperProfile() {
  const { t } = useTranslation();
  const profile = useAppStore((s) => s.unifiedProfile);
  const loading = useAppStore((s) => s.unifiedProfileLoading);
  const loadProfile = useAppStore((s) => s.loadUnifiedProfile);
  const exportMarkdown = useAppStore((s) => s.exportProfileMarkdown);
  const exportJson = useAppStore((s) => s.exportProfileJson);
  const setActiveView = useAppStore((s) => s.setActiveView);
  const learnedAffinities = useAppStore((s) => s.learnedAffinities);
  const antiTopics = useAppStore((s) => s.antiTopics);
  const lastLearnedTopic = useAppStore((s) => s.lastLearnedTopic);
  const [exportStatus, setExportStatus] = useState<string | null>(null);

  useEffect(() => {
    loadProfile();
  }, [loadProfile]);

  const handleDimensionAction = useCallback((action: string) => {
    switch (action) {
      case 'scan_infra':
      case 'scan_stack':
        cmd('ace_auto_discover').catch((e) => console.warn('SovereignProfile: auto-discover failed', e));
        break;
      case 'open_playbook':
        setActiveView('playbook');
        break;
      case 'open_settings':
        // Find and click the settings button in the header
        document.querySelector<HTMLButtonElement>('[data-settings-trigger]')?.click();
        break;
    }
  }, [setActiveView]);

  const handleExport = async (format: 'markdown' | 'json') => {
    try {
      const content = format === 'markdown' ? await exportMarkdown() : await exportJson();
      await window.navigator.clipboard.writeText(content);
      setExportStatus(format === 'markdown' ? t('profile.export.markdownCopied') : t('profile.export.jsonCopied'));
      setTimeout(() => setExportStatus(null), 2000);
    } catch {
      setExportStatus(t('profile.export.failed'));
      setTimeout(() => setExportStatus(null), 2000);
    }
  };
  const { infraDim, stackDim, skillsDim, prefsDim, ctxDim } = useMemo(() => {
    if (!profile) return { infraDim: { name: 'Infrastructure', ...EMPTY_DIM }, stackDim: { name: 'Stack', ...EMPTY_DIM }, skillsDim: { name: 'Skills', ...EMPTY_DIM }, prefsDim: { name: 'Preferences', ...EMPTY_DIM }, ctxDim: { name: 'Context', ...EMPTY_DIM } };
    const dims = profile.completeness.dimensions;
    return {
      infraDim: dims.find((d) => d.name === 'Infrastructure') || { name: 'Infrastructure', ...EMPTY_DIM },
      stackDim: dims.find((d) => d.name === 'Stack') || { name: 'Stack', ...EMPTY_DIM },
      skillsDim: dims.find((d) => d.name === 'Skills') || { name: 'Skills', ...EMPTY_DIM },
      prefsDim: dims.find((d) => d.name === 'Preferences') || { name: 'Preferences', ...EMPTY_DIM },
      ctxDim: dims.find((d) => d.name === 'Context') || { name: 'Context', ...EMPTY_DIM },
    };
  }, [profile]);

  const hasIntelligence = useMemo(() => {
    if (!profile) return false;
    const intel = profile.intelligence;
    return intel.skill_gaps.length > 0 || intel.optimization_opportunities.length > 0 || intel.infrastructure_mismatches.length > 0 || intel.ecosystem_alerts.length > 0;
  }, [profile]);

  if (loading && !profile) {
    return (
      <div className="bg-bg-secondary border border-border rounded-lg p-6">
        <div className="animate-pulse space-y-3" role="status" aria-label={t('action.loading', 'Loading')}>
          <div className="h-5 bg-white/5 rounded w-48" />
          <div className="h-3 bg-white/5 rounded w-72" />
          <div className="grid grid-cols-2 lg:grid-cols-3 gap-3 mt-4">
            {[1, 2, 3, 4, 5].map((i) => (
              <div key={i} className="h-24 bg-white/5 rounded-lg" />
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (!profile) return (
    <div className="bg-bg-secondary rounded-lg border border-red-500/30 p-6 text-center">
      <p className="text-sm text-red-400 mb-3">{t('profile.loadFailed')}</p>
      <button
        onClick={loadProfile}
        className="px-3 py-1.5 text-xs bg-red-500/10 text-red-400 rounded hover:bg-red-500/20 transition-colors"
      >
        {t('action.retry')}
      </button>
    </div>
  );

  const intel = profile.intelligence;

  return (
    <div className="bg-bg-secondary border border-border rounded-lg p-5 space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <CompletenessRing percentage={profile.completeness.overall_percentage} size={52} />
          <div>
            <h2 className="text-base font-semibold text-white">{profile.identity_summary}</h2>
            <p className="text-xs text-text-muted">
              {t('profile.percentComplete', { percent: Math.round(profile.completeness.overall_percentage) })}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {exportStatus && <span className="text-[10px] text-green-400">{exportStatus}</span>}
          <button
            onClick={() => handleExport('markdown')}
            aria-label={t('profile.copyProfile')}
            className="px-2.5 py-1 text-[10px] text-white bg-white/10 hover:bg-white/15 border border-white/20 rounded font-medium transition-colors"
          >
            {t('profile.copyProfile')}
          </button>
          <button
            onClick={() => handleExport('json')}
            aria-label={t('profile.exportJson')}
            className="px-2 py-1 text-[10px] text-text-secondary hover:text-white border border-border rounded transition-colors"
          >
            {t('profile.exportJson')}
          </button>
        </div>
      </div>

      {/* 5 Dimension Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
        {/* Infrastructure */}
        <DimensionCard dim={infraDim} onAction={handleDimensionAction}>
          {profile.infrastructure.gpu_tier !== 'none' && <p>{t('sovereignProfile.gpu', { tier: profile.infrastructure.gpu_tier })}</p>}
          <p>{t('sovereignProfile.llm', { tier: profile.infrastructure.llm_tier })}</p>
          {Object.keys(profile.infrastructure.cpu).length > 0 && (
            <p>{t('sovereignProfile.cpu', { model: profile.infrastructure.cpu.model || profile.infrastructure.cpu.name || t('sovereignProfile.detected') })}</p>
          )}
        </DimensionCard>

        {/* Stack */}
        <DimensionCard dim={stackDim} onAction={handleDimensionAction}>
          {profile.stack.primary_stack.length > 0 && (
            <p>{t('sovereignProfile.primaryStack', { stack: profile.stack.primary_stack.slice(0, 4).join(', ') })}</p>
          )}
          {profile.stack.selected_profiles.length > 0 && (
            <p>{t('sovereignProfile.profiles', { profiles: profile.stack.selected_profiles.join(', ') })}</p>
          )}
          <p>{t('sovereignProfile.dependenciesTracked', { count: profile.stack.dependencies.length })}</p>
        </DimensionCard>

        {/* Skills */}
        <DimensionCard dim={skillsDim} onAction={handleDimensionAction}>
          {profile.skills.top_affinities.length > 0 && (
            <p>{t('sovereignProfile.topAffinities', { topics: profile.skills.top_affinities.slice(0, 3).map((a) => a.topic).join(', ') })}</p>
          )}
          <p>
            {t('sovereignProfile.streetsProgress', { completed: profile.skills.playbook_progress.completed_lessons, total: profile.skills.playbook_progress.total_lessons })}
          </p>
        </DimensionCard>

        {/* Preferences */}
        <DimensionCard dim={prefsDim} onAction={handleDimensionAction}>
          {profile.preferences.interests.length > 0 && (
            <p>{t('sovereignProfile.interests', { interests: profile.preferences.interests.slice(0, 4).join(', ') })}</p>
          )}
          {profile.preferences.active_decisions.length > 0 && (
            <p>{t('sovereignProfile.activeDecisions', { count: profile.preferences.active_decisions.length })}</p>
          )}
        </DimensionCard>

        {/* Context */}
        <DimensionCard dim={ctxDim} onAction={handleDimensionAction}>
          <p>{t('sovereignProfile.projectsMonitored', { count: profile.context.projects_monitored })}</p>
          {profile.context.active_topics.length > 0 && (
            <p>{t('sovereignProfile.activeTopics', { topics: profile.context.active_topics.slice(0, 4).join(', ') })}</p>
          )}
        </DimensionCard>
      </div>

      {/* Learning Indicator */}
      <Suspense fallback={null}>
        <LearningIndicator
          learnedAffinities={learnedAffinities}
          antiTopics={antiTopics}
          lastLearnedTopic={lastLearnedTopic}
        />
      </Suspense>

      {/* Intelligence Section */}
      {hasIntelligence && (
        <div className="border-t border-border pt-4 space-y-3">
          <h3 className="text-xs font-medium text-text-secondary uppercase tracking-wider">{t('profile.intelligence')}</h3>

          {intel.skill_gaps.length > 0 && (
            <div>
              <h4 className="text-[11px] font-medium text-amber-400 mb-1">{t('profile.skillGaps', { count: intel.skill_gaps.length })}</h4>
              <div className="flex flex-wrap gap-1.5">
                {intel.skill_gaps.slice(0, 6).map((g) => (
                  <span key={g.dependency} className="px-2 py-0.5 text-[10px] bg-amber-500/10 text-amber-300 rounded-full">
                    {g.dependency}
                  </span>
                ))}
              </div>
            </div>
          )}

          {intel.optimization_opportunities.length > 0 && (
            <div>
              <h4 className="text-[11px] font-medium text-blue-400 mb-1">{t('profile.optimizations')}</h4>
              {intel.optimization_opportunities.slice(0, 3).map((o, i) => (
                <p key={i} className="text-[11px] text-text-secondary">
                  <span className="text-blue-300">{o.area}</span> — {o.suggestion}
                </p>
              ))}
            </div>
          )}

          {intel.infrastructure_mismatches.length > 0 && (
            <div>
              <h4 className="text-[11px] font-medium text-red-400 mb-1">{t('profile.infraMismatches')}</h4>
              {intel.infrastructure_mismatches.map((m, i) => (
                <p key={i} className="text-[11px] text-text-secondary">
                  <span className="text-red-300">{m.category}</span> — {m.issue}
                </p>
              ))}
            </div>
          )}

          {intel.ecosystem_alerts.length > 0 && (
            <div>
              <h4 className="text-[11px] font-medium text-purple-400 mb-1">{t('profile.ecosystemAlerts')}</h4>
              {intel.ecosystem_alerts.map((a, i) => (
                <p key={i} className="text-[11px] text-text-secondary">
                  <span className="text-purple-300">{a.from_tech} → {a.to_tech}</span> — {a.description}
                </p>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Developer DNA (on-demand) */}
      <DeveloperDnaSection />
    </div>
  );
});

