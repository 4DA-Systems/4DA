import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import type { EngineChoice } from '../../types/coach';

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function Spinner() {
  return (
    <div className="w-5 h-5 border-2 border-[#D4AF37] border-t-transparent rounded-full animate-spin" />
  );
}

function FitBar({ score }: { score: number }) {
  const pct = Math.min(Math.max(score, 0), 100);
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
        <div
          className="h-full bg-[#D4AF37] rounded-full transition-all duration-500"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-xs text-text-secondary font-mono w-10 text-right">{pct}%</span>
    </div>
  );
}

function EngineCard({
  engine,
  isPrimary,
}: {
  engine: EngineChoice;
  isPrimary: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div
      className={`flex-1 min-w-[260px] bg-bg-secondary rounded-xl p-5 space-y-4 border ${
        isPrimary ? 'border-[#D4AF37]/40' : 'border-border'
      }`}
    >
      {/* Header */}
      <div className="flex items-center gap-3">
        <span
          className={`w-8 h-8 rounded-lg flex items-center justify-center text-xs font-bold ${
            isPrimary
              ? 'bg-[#D4AF37]/20 text-[#D4AF37]'
              : 'bg-bg-tertiary text-text-secondary'
          }`}
        >
          {engine.engine_number}
        </span>
        <div className="flex-1 min-w-0">
          <h4 className="text-sm font-semibold text-white truncate">
            {engine.engine_name}
          </h4>
          <span className="text-[10px] uppercase tracking-wider text-[#666] font-medium">
            {isPrimary ? t('coach.engine.primaryEngine') : t('coach.engine.secondaryEngine')}
          </span>
        </div>
      </div>

      {/* Fit Score */}
      <div>
        <p className="text-[10px] text-[#666] uppercase tracking-wide mb-1">
          {t('coach.engine.fitScore')}
        </p>
        <FitBar score={engine.fit_score} />
      </div>

      {/* Metrics */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <p className="text-[10px] text-[#666] uppercase tracking-wide mb-0.5">
            {t('coach.engine.timeToFirstDollar')}
          </p>
          <p className="text-xs text-white font-medium">
            {engine.time_to_first_dollar}
          </p>
        </div>
        <div>
          <p className="text-[10px] text-[#666] uppercase tracking-wide mb-0.5">
            {t('coach.engine.revenueRange')}
          </p>
          <p className="text-xs text-white font-medium">
            {engine.revenue_range}
          </p>
        </div>
      </div>

      {/* Reasoning */}
      <div>
        <p className="text-[10px] text-[#666] uppercase tracking-wide mb-1">
          {t('coach.engine.reasoning')}
        </p>
        <p className="text-xs text-text-secondary leading-relaxed">
          {engine.reasoning}
        </p>
      </div>

      {/* Prerequisites */}
      {engine.prerequisites.length > 0 && (
        <div>
          <p className="text-[10px] text-[#666] uppercase tracking-wide mb-1">
            {t('coach.engine.prerequisites')}
          </p>
          <ul className="space-y-1">
            {engine.prerequisites.map((prereq, i) => (
              <li key={i} className="flex items-start gap-2 text-xs text-text-secondary">
                <span className="text-[#D4AF37] mt-0.5 flex-shrink-0">-</span>
                <span>{prereq}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function EngineRecommender() {
  const { t } = useTranslation();
  const recommendation = useAppStore((s) => s.engineRecommendation);
  const loading = useAppStore((s) => s.coachLoading);
  const recommendEngines = useAppStore((s) => s.recommendEngines);

  const handleAnalyze = useCallback(() => {
    recommendEngines();
  }, [recommendEngines]);

  return (
    <div className="space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold text-white">{t('coach.engine.title')}</h3>
          <p className="text-xs text-[#666] mt-0.5">
            {t('coach.engine.subtitle')}
          </p>
        </div>
        <button
          onClick={handleAnalyze}
          disabled={loading}
          className="px-4 py-2 text-sm font-medium bg-[#D4AF37] text-black rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {loading ? t('coach.engine.analyzing') : t('coach.engine.analyzeProfile')}
        </button>
      </div>

      {/* Loading State */}
      {loading && !recommendation && (
        <div className="flex items-center justify-center py-16">
          <div className="flex flex-col items-center gap-3">
            <Spinner />
            <p className="text-xs text-text-secondary">
              {t('coach.engine.analyzingProfile')}
            </p>
          </div>
        </div>
      )}

      {/* Recommendation Results */}
      {recommendation && (
        <div className="space-y-5">
          {/* Engine Cards */}
          <div className="flex gap-4 flex-wrap">
            <EngineCard engine={recommendation.primary_engine} isPrimary />
            <EngineCard engine={recommendation.secondary_engine} isPrimary={false} />
          </div>

          {/* Overall Reasoning */}
          <div className="bg-bg-secondary border border-border rounded-xl p-5 space-y-3">
            <div>
              <p className="text-[10px] text-[#666] uppercase tracking-wide mb-1">
                {t('coach.engine.overallAnalysis')}
              </p>
              <p className="text-xs text-text-secondary leading-relaxed">
                {recommendation.reasoning}
              </p>
            </div>

            {/* Profile Gaps */}
            {recommendation.profile_gaps.length > 0 && (
              <div>
                <p className="text-[10px] text-[#666] uppercase tracking-wide mb-1">
                  {t('coach.engine.profileGaps')}
                </p>
                <ul className="space-y-1">
                  {recommendation.profile_gaps.map((gap, i) => (
                    <li
                      key={i}
                      className="flex items-start gap-2 text-xs text-text-secondary"
                    >
                      <span className="text-[#F97316] mt-0.5 flex-shrink-0">*</span>
                      <span>{gap}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Empty State */}
      {!loading && !recommendation && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <div className="w-12 h-12 bg-[#D4AF37]/10 rounded-xl flex items-center justify-center mb-3">
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="#D4AF37"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <circle cx="12" cy="12" r="10" />
              <path d="M12 16v-4M12 8h.01" />
            </svg>
          </div>
          <p className="text-sm text-text-secondary max-w-sm">
            {t('coach.engine.emptyDescription')}
          </p>
        </div>
      )}
    </div>
  );
}
