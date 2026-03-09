import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../lib/commands';
import { useAppStore } from '../store';
import { DimensionBar, StatusRow, RecommendationItem } from './calibration/CalibrationComponents';
import { gradeColor } from './calibration/calibration-utils';
import type { CalibrationResult, Recommendation } from '../types/calibration';
import type { PullProgress } from './calibration/calibration-utils';

export function CalibrationView() {
  const { t } = useTranslation();
  const [result, setResult] = useState<CalibrationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [prevGrade, setPrevGrade] = useState<string | null>(null);
  const [pullProgress, setPullProgress] = useState<PullProgress | null>(null);
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  const hasAutoRun = useRef(false);
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const setActiveView = useAppStore(s => s.setActiveView);

  const runCalibration = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await cmd('run_calibration');
      if (result) setPrevGrade(result.grade);
      setResult(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [result]);

  // Auto-run calibration on mount
  useEffect(() => {
    if (!hasAutoRun.current) {
      hasAutoRun.current = true;
      runCalibration();
    }
  }, [runCalibration]);

  // Listen for Ollama pull progress
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<PullProgress>('ollama-pull-progress', (event) => {
      setPullProgress(event.payload);
      if (event.payload.done) {
        setActionInProgress(null);
        setTimeout(() => setPullProgress(null), 1500);
        // Auto re-calibrate after model pull completes
        setTimeout(() => runCalibration(), 2000);
      }
    }).then(fn => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [runCalibration]);

  const handleAction = async (rec: Recommendation) => {
    if (!rec.action_type || actionInProgress) return;

    switch (rec.action_type) {
      case 'pull_embedding_model': {
        setActionInProgress('pull_embedding_model');
        try {
          await cmd('pull_ollama_model', {
            model: result?.rig_requirements.recommended_model || 'nomic-embed-text',
            baseUrl: null,
          });
        } catch (e) {
          setError(t('calibration.modelPullFailed', { error: e instanceof Error ? e.message : String(e) }));
          setActionInProgress(null);
          setPullProgress(null);
        }
        break;
      }
      case 'open_settings_interests':
      case 'open_settings_stacks': {
        setShowSettings(true);
        break;
      }
      case 'auto_detect_stacks': {
        setActionInProgress('auto_detect_stacks');
        try {
          const detected = await cmd('detect_stack_profiles');
          if (detected.length > 0) {
            const topIds = detected.slice(0, 3).map(d => d.profile_id);
            await cmd('set_selected_stacks', { profileIds: topIds });
            // Auto re-calibrate after stack detection
            await runCalibration();
          } else {
            // No auto-detection — open settings as fallback
            setShowSettings(true);
          }
        } catch (e) {
          setError(t('calibration.stackDetectionFailed', { error: e instanceof Error ? e.message : String(e) }));
        } finally {
          setActionInProgress(null);
        }
        break;
      }
      case 'install_ollama': {
        import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl('https://ollama.com/download'));
        break;
      }
      case 'give_feedback': {
        setActiveView('results');
        break;
      }
    }
  };

  const gradeImproved = prevGrade && result && prevGrade !== result.grade;

  return (
    <div className="p-6 max-w-[720px]">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="m-0 text-xl font-semibold text-white">
            {t('calibration.title')}
          </h2>
          <p className="mt-1 mb-0 text-text-secondary text-[13px]">
            {t('calibration.subtitle')}
          </p>
        </div>
        <button
          onClick={runCalibration}
          disabled={loading}
          aria-label={loading ? t('calibration.running') : t('calibration.reCalibrate')}
          className={`px-5 py-2 border-none rounded-md text-[13px] font-semibold font-[Inter,sans-serif] ${
            loading
              ? 'bg-border text-text-muted cursor-not-allowed'
              : 'bg-white text-bg-primary cursor-pointer'
          }`}
        >
          {loading ? t('calibration.running') : t('calibration.reCalibrate')}
        </button>
      </div>

      {error && (
        <div className="p-3 bg-[#1a0000] border border-error rounded-md text-error text-[13px] mb-4" role="alert">
          {error}
        </div>
      )}

      {/* Grade improvement banner */}
      {gradeImproved && (
        <div className="p-3 bg-[#0a1a0a] border border-success rounded-md mb-4 flex items-center gap-2" role="status" aria-label={t('calibration.gradeImproved', { prev: prevGrade, next: result.grade })}>
          <span className="text-lg">&#x2191;</span>
          <span className="text-[13px] text-success font-semibold">
            {t('calibration.gradeImproved', { prev: prevGrade, next: result.grade })}
          </span>
        </div>
      )}

      {/* Pull progress bar */}
      {pullProgress && (
        <div className="mb-4">
          <div className="flex justify-between mb-1">
            <span className="text-[11px] text-text-secondary">
              {t('calibration.pulling', { model: pullProgress.model })}
            </span>
            <span className="text-[11px] text-accent-gold font-mono">
              {pullProgress.done ? t('calibration.pullDone') : `${pullProgress.percent}%`}
            </span>
          </div>
          <div
            className="h-1 bg-border rounded-sm overflow-hidden"
            role="progressbar"
            aria-valuenow={pullProgress.done ? 100 : pullProgress.percent}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={t('calibration.pulling', { model: pullProgress.model })}
          >
            <div
              className={`h-full rounded-sm transition-[width] duration-300 ease-in-out ${
                pullProgress.done ? 'bg-success' : 'bg-accent-gold'
              }`}
              style={{ width: `${pullProgress.done ? 100 : pullProgress.percent}%` }}
            />
          </div>
          <div className="text-[10px] text-text-muted mt-0.5">
            {pullProgress.status}
          </div>
        </div>
      )}

      {result && (
        <>
          {/* Grade Card */}
          <div className="flex gap-4 mb-5">
            <div
              className="flex-[0_0_120px] bg-bg-secondary border border-border rounded-lg p-5 text-center"
              role="status"
              aria-label={t('calibration.ariaGradeScore', { grade: result.grade, score: result.grade_score })}
            >
              <div className="text-5xl font-bold font-mono" style={{ color: gradeColor(result.grade) }}>
                {result.grade}
              </div>
              <div className="text-xs text-text-muted mt-1">
                {result.grade_score}/100
              </div>
            </div>
            <div className="flex-1 bg-bg-secondary border border-border rounded-lg p-4">
              <div className="flex flex-col gap-2">
                <DimensionBar label={t('calibration.dimension.infrastructure')} score={result.infrastructure_score} />
                <DimensionBar label={t('calibration.dimension.context')} score={result.context_richness_score} />
                <DimensionBar label={t('calibration.dimension.signalCoverage')} score={result.signal_coverage_score} />
                <DimensionBar label={t('calibration.dimension.discrimination')} score={result.discrimination_score} />
              </div>
              {result.active_signal_axes.length > 0 && (
                <div className="flex gap-1 mt-2.5 flex-wrap">
                  {result.active_signal_axes.map(axis => (
                    <span key={axis} className="px-2 py-0.5 bg-bg-tertiary rounded-[10px] text-[10px] text-success font-mono">
                      {axis}
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Rig Requirements */}
          <div className="bg-bg-secondary border border-border rounded-lg p-4 mb-5">
            <h3 className="m-0 mb-3 text-sm font-semibold text-white">
              {t('calibration.rigStatus')}
            </h3>
            <div className="grid grid-cols-2 gap-2">
              <StatusRow
                label={t('calibration.label.ollama')}
                ok={result.rig_requirements.ollama_running}
                detail={result.rig_requirements.ollama_running ? t('calibration.ollamaRunning') : t('calibration.ollamaNotDetected')}
              />
              <StatusRow
                label={t('calibration.label.embeddings')}
                ok={result.rig_requirements.embedding_available}
                detail={result.rig_requirements.embedding_model || t('calibration.notAvailable')}
              />
              <StatusRow
                label={t('calibration.gradeACapable')}
                ok={result.rig_requirements.can_reach_grade_a}
                detail={result.rig_requirements.can_reach_grade_a ? t('calibration.yes') : t('calibration.needsSetup')}
              />
              <StatusRow
                label={t('calibration.label.recommendedRam')}
                ok={true}
                detail={`${result.rig_requirements.estimated_ram_gb} ${t('calibration.gbUnit')}`}
              />
            </div>

            {!result.rig_requirements.can_reach_grade_a && (
              <div className="mt-3 p-3 bg-bg-tertiary rounded-md">
                <div className="text-xs font-semibold text-accent-gold mb-1.5">
                  {t('calibration.toReachGradeA')}
                </div>
                {result.rig_requirements.grade_a_requirements.map((req, i) => (
                  <div key={i} className="text-xs text-text-secondary py-0.5 pl-3">
                    {req}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Recommendations with action buttons */}
          {result.recommendations.length > 0 && (
            <div className="bg-bg-secondary border border-border rounded-lg p-4">
              <h3 className="m-0 mb-1 text-sm font-semibold text-white">
                {result.grade.startsWith('A')
                  ? t('calibration.maintainingGradeA')
                  : t('calibration.upgradePath')}
              </h3>
              <p className="m-0 mb-3 text-[11px] text-text-muted">
                {t('calibration.recommendationsHint')}
              </p>
              {result.recommendations.map((rec, i) => (
                <RecommendationItem
                  key={i}
                  rec={rec}
                  index={i}
                  actionInProgress={actionInProgress}
                  onAction={handleAction}
                />
              ))}
            </div>
          )}

          {/* Grade A achieved */}
          {result.recommendations.length === 0 && (
            <div className="bg-[#0a1a0a] border border-success rounded-lg p-5 text-center" role="status" aria-label={t('calibration.fullyCalibrated')}>
              <div className="text-2xl mb-2">&#x2713;</div>
              <div className="text-sm font-semibold text-success">
                {t('calibration.fullyCalibrated')}
              </div>
              <div className="text-xs text-text-secondary mt-1">
                {t('calibration.allScoringReady')}
              </div>
            </div>
          )}
        </>
      )}

      {!result && loading && (
        <div className="text-center py-[60px] px-5 text-text-secondary text-[13px]" role="status" aria-label={t('calibration.analyzing')}>
          {t('calibration.analyzing')}
        </div>
      )}
    </div>
  );
}
