import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../../lib/commands';
import type { CalibrationResult, Recommendation } from '../../types/calibration';

interface CalibrationStepProps {
  isAnimating: boolean;
  onComplete: () => void;
  onBack: () => void;
}

interface PullProgress {
  model: string;
  status: string;
  percent: number;
  done: boolean;
}

export function CalibrationStep({ isAnimating, onComplete, onBack }: CalibrationStepProps) {
  const { t } = useTranslation();
  const [result, setResult] = useState<CalibrationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pullProgress, setPullProgress] = useState<PullProgress | null>(null);
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  const hasAutoRun = useRef(false);

  const runCalibration = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setResult(await cmd('run_calibration'));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!hasAutoRun.current) {
      hasAutoRun.current = true;
      runCalibration();
    }
  }, [runCalibration]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<PullProgress>('ollama-pull-progress', (event) => {
      setPullProgress(event.payload);
      if (event.payload.done) {
        setActionInProgress(null);
        setTimeout(() => setPullProgress(null), 1500);
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
          setError(t('calibration.onboarding.pullFailed', { error: e instanceof Error ? e.message : String(e) }));
          setActionInProgress(null);
          setPullProgress(null);
        }
        break;
      }
      case 'auto_detect_stacks': {
        setActionInProgress('auto_detect_stacks');
        try {
          const detected = await cmd('detect_stack_profiles');
          if (detected.length > 0) {
            await cmd('set_selected_stacks', { profileIds: detected.slice(0, 3).map(d => d.profile_id) });
            await runCalibration();
          }
        } catch {
          // Non-critical
        } finally {
          setActionInProgress(null);
        }
        break;
      }
    }
  };

  const gradeColor = (grade: string) => {
    if (grade.startsWith('A')) return '#22C55E';
    if (grade.startsWith('B')) return '#D4AF37';
    if (grade.startsWith('C')) return '#F59E0B';
    return '#EF4444';
  };

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h1 className="text-2xl font-semibold text-white mb-2 text-center">{t('calibration.title')}</h1>
      <p className="text-sm text-text-secondary mb-6 text-center">
        {t('calibration.onboarding.subtitle')}
      </p>

      {error && (
        <div style={{ padding: 10, background: '#1a0000', border: '1px solid #EF4444', borderRadius: 6, color: '#EF4444', fontSize: 12, marginBottom: 12 }}>
          {error}
        </div>
      )}

      {pullProgress && (
        <div style={{ marginBottom: 12 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 2 }}>
            <span style={{ fontSize: 11, color: '#A0A0A0' }}>{t('calibration.pulling', { model: pullProgress.model })}</span>
            <span style={{ fontSize: 11, color: '#D4AF37', fontFamily: 'JetBrains Mono, monospace' }}>
              {pullProgress.done ? t('calibration.pullDone') : `${pullProgress.percent}%`}
            </span>
          </div>
          <div style={{ height: 4, background: '#2A2A2A', borderRadius: 2, overflow: 'hidden' }}>
            <div style={{ height: '100%', width: `${pullProgress.done ? 100 : pullProgress.percent}%`, background: pullProgress.done ? '#22C55E' : '#D4AF37', borderRadius: 2, transition: 'width 0.3s ease' }} />
          </div>
        </div>
      )}

      {loading && !result && (
        <div style={{ textAlign: 'center', padding: '40px 0' }}>
          <div style={{ width: 24, height: 24, border: '2px solid #2A2A2A', borderTopColor: '#D4AF37', borderRadius: '50%', animation: 'spin 0.8s linear infinite', margin: '0 auto 12px' }} />
          <div style={{ color: '#A0A0A0', fontSize: 13 }}>{t('calibration.onboarding.analyzing')}</div>
          <div style={{ color: '#8A8A8A', fontSize: 11, marginTop: 4 }}>{t('calibration.onboarding.analyzingDetail')}</div>
        </div>
      )}

      {result && (
        <>
          {/* Grade + dimension bars */}
          <div style={{ display: 'flex', gap: 12, marginBottom: 16 }}>
            <div style={{ flex: '0 0 90px', background: '#141414', border: '1px solid #2A2A2A', borderRadius: 8, padding: 14, textAlign: 'center' }}>
              <div style={{ fontSize: 36, fontWeight: 700, color: gradeColor(result.grade), fontFamily: 'JetBrains Mono, monospace' }}>
                {result.grade}
              </div>
              <div style={{ fontSize: 11, color: '#8A8A8A' }}>{result.grade_score}/100</div>
            </div>
            <div style={{ flex: 1, background: '#141414', border: '1px solid #2A2A2A', borderRadius: 8, padding: 12 }}>
              {[
                { label: t('calibration.dimension.infrastructure'), score: result.infrastructure_score },
                { label: t('calibration.dimension.context'), score: result.context_richness_score },
                { label: t('calibration.dimension.signalCoverage'), score: result.signal_coverage_score },
                { label: t('calibration.dimension.discrimination'), score: result.discrimination_score },
              ].map(d => {
                const pct = Math.round((d.score / 25) * 100);
                const c = pct >= 80 ? '#22C55E' : pct >= 50 ? '#D4AF37' : pct >= 25 ? '#F59E0B' : '#EF4444';
                return (
                  <div key={d.label} style={{ marginBottom: 6 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 1 }}>
                      <span style={{ fontSize: 10, color: '#A0A0A0' }}>{d.label}</span>
                      <span style={{ fontSize: 10, color: c, fontFamily: 'JetBrains Mono, monospace' }}>{d.score}/25</span>
                    </div>
                    <div style={{ height: 3, background: '#2A2A2A', borderRadius: 2, overflow: 'hidden' }}>
                      <div style={{ height: '100%', width: `${pct}%`, background: c, borderRadius: 2 }} />
                    </div>
                  </div>
                );
              })}
              {result.active_signal_axes.length > 0 && (
                <div style={{ display: 'flex', gap: 3, marginTop: 6, flexWrap: 'wrap' }}>
                  {result.active_signal_axes.map(a => (
                    <span key={a} style={{ padding: '1px 6px', background: '#1F1F1F', borderRadius: 8, fontSize: 9, color: '#22C55E', fontFamily: 'JetBrains Mono, monospace' }}>{a}</span>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Actionable recommendations (only P0/P1) */}
          {result.recommendations.filter(r => r.action_type && r.priority !== 'P2').length > 0 && (
            <div style={{ background: '#141414', border: '1px solid #2A2A2A', borderRadius: 8, padding: 12, marginBottom: 16 }}>
              {result.recommendations.filter(r => r.action_type && r.priority !== 'P2').map((rec, i) => (
                <div key={i} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '6px 0', borderTop: i > 0 ? '1px solid #1F1F1F' : 'none' }}>
                  <div>
                    <span style={{ fontSize: 12, color: '#FFFFFF', fontWeight: 500 }}>{rec.title}</span>
                    <span style={{ fontSize: 10, color: rec.priority === 'P0' ? '#EF4444' : '#F59E0B', marginLeft: 6, fontFamily: 'JetBrains Mono, monospace' }}>{rec.priority}</span>
                  </div>
                  {rec.action_type && (
                    <button
                      onClick={() => handleAction(rec)}
                      disabled={!!actionInProgress}
                      style={{
                        padding: '3px 10px', background: actionInProgress === rec.action_type ? '#2A2A2A' : '#D4AF37',
                        color: actionInProgress === rec.action_type ? '#8A8A8A' : '#0A0A0A',
                        border: 'none', borderRadius: 4, fontSize: 10, fontWeight: 600, cursor: actionInProgress ? 'not-allowed' : 'pointer',
                      }}
                    >
                      {actionInProgress === rec.action_type ? t('calibration.action.working') : t('calibration.action.fix')}
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}
        </>
      )}

      {/* No-result explanation */}
      {!loading && !result && !error && (
        <div style={{ textAlign: 'center', padding: '24px 0', color: '#A0A0A0', fontSize: 13 }}>
          <p>{t('calibration.onboarding.noContent')}</p>
          <p style={{ fontSize: 11, color: '#8A8A8A', marginTop: 4 }}>{t('calibration.onboarding.noContentHint')}</p>
        </div>
      )}

      {/* Navigation */}
      <div className="flex justify-between mt-6">
        <button onClick={onBack} className="px-4 py-2 text-sm text-text-secondary hover:text-white transition-colors">
          {t('onboarding.nav.back')}
        </button>
        <button
          onClick={onComplete}
          className="px-6 py-2 bg-orange-500 hover:bg-orange-600 text-white text-sm font-medium rounded-lg transition-colors"
        >
          {result ? t('onboarding.interests.finishSetup') : t('onboarding.nav.skipForNow')}
        </button>
      </div>
    </div>
  );
}
