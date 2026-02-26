import { useEffect, useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import type { CoachMessage } from '../../types/coach';

function ProgressBar({ percentage, label }: { percentage: number; label: string }) {
  const pct = Math.min(Math.max(percentage, 0), 100);
  return (
    <div>
      <div className="flex items-center justify-between mb-1">
        <span className="text-xs text-[#A0A0A0]">{label}</span>
        <span className="text-xs text-[#A0A0A0] font-mono">{Math.round(pct)}%</span>
      </div>
      <div className="h-1.5 bg-[#1F1F1F] rounded-full overflow-hidden">
        <div className="h-full bg-[#D4AF37] rounded-full transition-all duration-500" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

function DashboardCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-5">
      <p className="text-[10px] text-[#666666] uppercase tracking-wide font-medium mb-3">{title}</p>
      {children}
    </div>
  );
}

function MarkdownBlock({ content }: { content: string }) {
  return (
    <div className="space-y-1.5">
      {content.split('\n').map((line, i) => {
        const t = line.trim();
        if (!t) return null;
        if (t.startsWith('## ')) return <h4 key={i} className="text-sm font-semibold text-white mt-3 mb-1">{t.slice(3)}</h4>;
        if (t.startsWith('# ')) return <h3 key={i} className="text-base font-bold text-white mt-4 mb-1.5">{t.slice(2)}</h3>;
        if (/^[-*] /.test(t)) return <p key={i} className="text-xs text-[#A0A0A0] leading-relaxed pl-3">- {t.slice(2)}</p>;
        return <p key={i} className="text-xs text-[#A0A0A0] leading-relaxed">{t}</p>;
      })}
    </div>
  );
}

const PROFILE_CATEGORIES = [
  { key: 'hardware', label: 'Hardware' },
  { key: 'software', label: 'Software' },
  { key: 'skills', label: 'Skills' },
  { key: 'goals', label: 'Goals' },
  { key: 'legal', label: 'Legal / Business' },
];

export function ProgressDashboard() {
  const { t } = useTranslation();
  const { nudges, loading, messages, playbookProgress, profileCompleteness } = useAppStore(
    useShallow((s) => ({
      nudges: s.coachNudges,
      loading: s.coachLoading,
      messages: s.coachMessages,
      playbookProgress: s.playbookProgress,
      profileCompleteness: s.profileCompleteness,
    })),
  );

  const dismissNudge = useAppStore((s) => s.dismissNudge);
  const progressCheckIn = useAppStore((s) => s.progressCheckIn);
  const loadNudges = useAppStore((s) => s.loadCoachNudges);
  const [checkInDone, setCheckInDone] = useState(false);

  useEffect(() => { loadNudges(); }, [loadNudges]);

  const handleCheckIn = useCallback(async () => {
    await progressCheckIn();
    setCheckInDone(true);
  }, [progressCheckIn]);

  const handleDismiss = useCallback((id: number) => { dismissNudge(id); }, [dismissNudge]);

  const playbookPct = playbookProgress?.overall_percentage ?? 0;
  const profilePct = profileCompleteness?.percentage ?? 0;
  const filledSet = new Set(
    PROFILE_CATEGORIES.map((c) => c.key).filter((k) => !profileCompleteness?.missing.includes(k)),
  );
  const checkInMessages: CoachMessage[] = messages.filter(
    (m) => m.session_id === 'progress' && m.role === 'assistant',
  );
  const latestCheckIn = checkInMessages.length > 0 ? checkInMessages[checkInMessages.length - 1] : null;

  return (
    <div className="space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold text-white">{t('coach.progress.title')}</h3>
          <p className="text-xs text-[#666666] mt-0.5">{t('coach.progress.subtitle')}</p>
        </div>
        <button
          onClick={handleCheckIn}
          disabled={loading}
          className="px-4 py-2 text-sm font-medium bg-[#D4AF37] text-black rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {loading ? t('coach.progress.checkingIn') : t('coach.progress.runCheckIn')}
        </button>
      </div>

      {/* Dashboard Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <DashboardCard title={t('coach.progress.playbookProgress')}>
          <ProgressBar percentage={playbookPct} label={t('coach.progress.overallCompletion')} />
          {playbookProgress?.modules && playbookProgress.modules.length > 0 ? (
            <div className="mt-3 space-y-2">
              {playbookProgress.modules.slice(0, 4).map((mod) => (
                <div key={mod.module_id} className="flex items-center gap-2">
                  <span className="w-6 h-6 rounded flex items-center justify-center text-[9px] font-bold bg-[#1F1F1F] text-[#A0A0A0] flex-shrink-0">
                    {mod.module_id}
                  </span>
                  <div className="flex-1 h-1 bg-[#1F1F1F] rounded-full overflow-hidden">
                    <div className="h-full bg-[#22C55E] rounded-full" style={{ width: `${mod.percentage}%` }} />
                  </div>
                  <span className="text-[10px] text-[#666666] font-mono w-8 text-right">{Math.round(mod.percentage)}%</span>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-[10px] text-[#666666] mt-2 italic">{t('coach.progress.startPlaybook')}</p>
          )}
        </DashboardCard>

        <DashboardCard title={t('coach.progress.profileCompleteness')}>
          <ProgressBar percentage={profilePct} label={t('coach.progress.sovereignProfile')} />
          <div className="mt-3 space-y-1.5">
            {PROFILE_CATEGORIES.map((cat) => {
              const pct = profileCompleteness ? (filledSet.has(cat.key) ? 100 : 0) : 0;
              return (
                <div key={cat.key} className="flex items-center justify-between">
                  <span className="text-[10px] text-[#666666]">{cat.label}</span>
                  <div className="flex items-center gap-2">
                    <div className="w-16 h-1 bg-[#1F1F1F] rounded-full overflow-hidden">
                      <div className="h-full bg-[#D4AF37] rounded-full" style={{ width: `${pct}%` }} />
                    </div>
                    <span className="text-[10px] text-[#666666] font-mono w-8 text-right">{pct}%</span>
                  </div>
                </div>
              );
            })}
          </div>
        </DashboardCard>

        <DashboardCard title={t('coach.progress.activeNudges')}>
          {nudges.length > 0 ? (
            <div className="space-y-2 max-h-[200px] overflow-y-auto">
              {nudges.map((nudge) => (
                <div key={nudge.id} className="flex items-start gap-2 bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg px-3 py-2">
                  <span className="text-[#D4AF37] mt-0.5 flex-shrink-0 text-xs">*</span>
                  <p className="text-xs text-[#A0A0A0] flex-1 leading-relaxed">{nudge.content}</p>
                  <button
                    onClick={() => handleDismiss(nudge.id)}
                    className="text-[10px] text-[#666666] hover:text-[#A0A0A0] transition-colors flex-shrink-0"
                    aria-label={t('coach.progress.dismissNudge')}
                  >
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <line x1="18" y1="6" x2="6" y2="18" />
                      <line x1="6" y1="6" x2="18" y2="18" />
                    </svg>
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-[10px] text-[#666666] italic">{t('coach.progress.noNudges')}</p>
          )}
        </DashboardCard>
      </div>

      {/* Check-In Results */}
      {checkInDone && latestCheckIn && (
        <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-5">
          <div className="flex items-center justify-between mb-3">
            <p className="text-[10px] text-[#666666] uppercase tracking-wide font-medium">{t('coach.progress.checkInResults')}</p>
            <span className="text-[10px] text-[#666666]">{new Date(latestCheckIn.created_at).toLocaleString()}</span>
          </div>
          <MarkdownBlock content={latestCheckIn.content} />
        </div>
      )}

      {/* Loading indicator */}
      {loading && (
        <div className="flex items-center gap-3 py-4">
          <div className="w-4 h-4 border-2 border-[#D4AF37] border-t-transparent rounded-full animate-spin" />
          <span className="text-xs text-[#A0A0A0]">{t('coach.progress.runningCheckIn')}</span>
        </div>
      )}
    </div>
  );
}
