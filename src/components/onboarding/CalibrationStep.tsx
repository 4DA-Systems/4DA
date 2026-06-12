// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../../lib/commands';
import { useAppStore } from '../../store';
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

// Friendly labels for the scoring signal axes (backend IDs from probes_engine.rs:
// context/interest/ace/learned/dependency). Used as i18n fallbacks so the calibrate
// screen never surfaces the raw internal axis IDs to the user.
const axisFallback: Record<string, string> = {
  context: 'Project context',
  interest: 'Your interests',
  ace: 'Auto-discovery',
  learned: 'Learned signals',
  dependency: 'Dependencies',
};

export function CalibrationStep({ isAnimating, onComplete, onBack }: CalibrationStepProps) {
  const { t } = useTranslation();
  const embeddingMode = useAppStore(s => s.embeddingMode);
  // Setup-complete counts come from the persisted backend profile (the same
  // source Settings reads) — NOT optimistic frontend store state, which drifted
  // and produced fabricated counts (e.g. "16 interests" with 0 persisted).
  const [setupCounts, setSetupCounts] = useState<{ tech: number; interests: number } | null>(null);
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
      void runCalibration();
    }
  }, [runCalibration]);

  // Pull the real, persisted profile counts for the "Setup complete" summary.
  useEffect(() => {
    let cancelled = false;
    void cmd('get_user_context')
      .then((ctx) => {
        if (cancelled) return;
        setSetupCounts({
          tech: ctx?.tech_stack?.length ?? 0,
          interests: ctx?.interests?.length ?? 0,
        });
      })
      .catch(() => { if (!cancelled) setSetupCounts({ tech: 0, interests: 0 }); });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<PullProgress>('ollama-pull-progress', (event) => {
      setPullProgress(event.payload);
      if (event.payload.done) {
        setActionInProgress(null);
        setTimeout(() => setPullProgress(null), 1500);
        setTimeout(() => { void runCalibration(); }, 2000);
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

  // Recommendation action_types this onboarding step can actually perform.
  // Others (e.g. give_feedback, open_settings_*) have no meaning yet — there's no
  // content to act on mid-onboarding — so we show them as guidance without a
  // button that would silently do nothing.
  const ONBOARDING_ACTIONABLE = ['pull_embedding_model', 'auto_detect_stacks'];

  const gradeColor = (grade: string) => {
    if (grade.startsWith('A')) return 'var(--color-success)';
    if (grade.startsWith('B')) return 'var(--color-accent-gold)';
    if (grade.startsWith('C')) return 'var(--color-amber-500)';
    return 'var(--color-error)';
  };

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h1 className="text-2xl font-semibold text-text-primary mb-2 text-center">{t('calibration.title')}</h1>
      <p className="text-sm text-text-secondary mb-6 text-center">
        {t('calibration.onboarding.subtitle')}
      </p>

      {error && (
        <div style={{ padding: 10, background: 'color-mix(in srgb, var(--color-error) 12%, var(--color-bg-primary))', border: '1px solid var(--color-error)', borderRadius: 6, color: 'var(--color-error)', fontSize: 12, marginBottom: 12 }}>
          {error}
        </div>
      )}

      {pullProgress && (
        <div style={{ marginBottom: 12 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 2 }}>
            <span style={{ fontSize: 11, color: 'var(--color-text-secondary)' }}>{t('calibration.pulling', { model: pullProgress.model })}</span>
            <span style={{ fontSize: 11, color: 'var(--color-accent-gold)', fontFamily: 'JetBrains Mono, monospace' }}>
              {pullProgress.done ? t('calibration.pullDone') : `${pullProgress.percent}%`}
            </span>
          </div>
          <div style={{ height: 4, background: 'var(--color-border)', borderRadius: 2, overflow: 'hidden' }}>
            <div style={{ height: '100%', width: `${pullProgress.done ? 100 : pullProgress.percent}%`, background: pullProgress.done ? 'var(--color-success)' : 'var(--color-accent-gold)', borderRadius: 2, transition: 'width 0.3s ease' }} />
          </div>
        </div>
      )}

      {loading && !result && (
        <div style={{ textAlign: 'center', padding: '40px 0' }}>
          <div style={{ width: 24, height: 24, border: '2px solid var(--color-border)', borderTopColor: 'var(--color-accent-gold)', borderRadius: '50%', animation: 'spin 0.8s linear infinite', margin: '0 auto 12px' }} />
          <div style={{ color: 'var(--color-text-secondary)', fontSize: 13 }}>{t('calibration.onboarding.analyzing')}</div>
          <div style={{ color: 'var(--color-text-muted)', fontSize: 11, marginTop: 4 }}>{t('calibration.onboarding.analyzingDetail')}</div>
        </div>
      )}

      {result && (
        <>
          {/* Grade + dimension bars */}
          <div style={{ display: 'flex', gap: 12, marginBottom: 16 }}>
            <div style={{ flex: '0 0 90px', background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)', borderRadius: 8, padding: 14, textAlign: 'center' }}>
              <div style={{ fontSize: 36, fontWeight: 700, color: gradeColor(result.grade), fontFamily: 'JetBrains Mono, monospace' }}>
                {result.grade}
              </div>
              <div style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>{result.grade_score}/100</div>
            </div>
            <div style={{ flex: 1, background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)', borderRadius: 8, padding: 12 }}>
              {[
                { label: t('calibration.dimension.infrastructure'), score: result.infrastructure_score },
                { label: t('calibration.dimension.context'), score: result.context_richness_score },
                { label: t('calibration.dimension.signalCoverage'), score: result.signal_coverage_score },
                { label: t('calibration.dimension.discrimination'), score: result.discrimination_score },
              ].map(d => {
                const pct = Math.round((d.score / 25) * 100);
                const c = pct >= 80 ? 'var(--color-success)' : pct >= 50 ? 'var(--color-accent-gold)' : pct >= 25 ? 'var(--color-amber-500)' : 'var(--color-error)';
                return (
                  <div key={d.label} style={{ marginBottom: 6 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 1 }}>
                      <span style={{ fontSize: 10, color: 'var(--color-text-secondary)' }}>{d.label}</span>
                      <span style={{ fontSize: 10, color: c, fontFamily: 'JetBrains Mono, monospace' }}>{d.score}/25</span>
                    </div>
                    <div style={{ height: 3, background: 'var(--color-border)', borderRadius: 2, overflow: 'hidden' }}>
                      <div style={{ height: '100%', width: `${pct}%`, background: c, borderRadius: 2 }} />
                    </div>
                  </div>
                );
              })}
              {result.active_signal_axes.length > 0 && (
                <div style={{ display: 'flex', gap: 3, marginTop: 6, flexWrap: 'wrap' }}>
                  {result.active_signal_axes.map(a => (
                    <span key={a} style={{ padding: '1px 6px', background: 'var(--color-bg-tertiary)', borderRadius: 8, fontSize: 9, color: 'var(--color-success)' }}>
                      {t(`calibration.axis.${a}`, axisFallback[a] ?? a)}
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Honest day-one framing: some signals (learned/dependency) can't fire until
              you've used 4DA, so a fresh setup grades lower by design. Say so. */}
          {result.grade_score < 70 && (
            <p style={{ fontSize: 11, color: 'var(--color-text-muted)', textAlign: 'center', marginBottom: 16, marginTop: -4 }}>
              {t('calibration.onboarding.gradeStartingPoint')}
            </p>
          )}

          {/* Actionable recommendations (only P0/P1) */}
          {result.recommendations.filter(r => r.action_type && r.priority !== 'P2').length > 0 && (
            <div style={{ background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)', borderRadius: 8, padding: 12, marginBottom: 16 }}>
              {result.recommendations.filter(r => r.action_type && r.priority !== 'P2').map((rec, i) => (
                <div key={i} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '6px 0', borderTop: i > 0 ? '1px solid var(--color-bg-tertiary)' : 'none' }}>
                  <div>
                    {/* A colored dot conveys urgency without leaking the internal
                        "P0"/"P1" priority code to the user. */}
                    <span aria-hidden="true" style={{ display: 'inline-block', width: 6, height: 6, borderRadius: '50%', background: rec.priority === 'P0' ? 'var(--color-error)' : 'var(--color-amber-500)', marginRight: 6, verticalAlign: 'middle' }} />
                    <span style={{ fontSize: 12, color: 'var(--color-text-primary)', fontWeight: 500 }}>{rec.title}</span>
                  </div>
                  {rec.action_type && ONBOARDING_ACTIONABLE.includes(rec.action_type) && (
                    <button
                      onClick={() => { void handleAction(rec); }}
                      disabled={!!actionInProgress}
                      style={{
                        padding: '3px 10px', background: actionInProgress === rec.action_type ? 'var(--color-border)' : 'var(--color-accent-gold)',
                        color: actionInProgress === rec.action_type ? 'var(--color-text-muted)' : 'var(--color-bg-primary)',
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
        <div style={{ textAlign: 'center', padding: '24px 0', color: 'var(--color-text-secondary)', fontSize: 13 }}>
          <p>{t('calibration.onboarding.noContent')}</p>
          <p style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 4 }}>{t('calibration.onboarding.noContentHint')}</p>
        </div>
      )}

      {/* Setup summary */}
      {result && (
        <div style={{ background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)', borderRadius: 8, padding: 12, marginBottom: 16 }}>
          <p style={{ fontSize: 11, color: 'var(--color-text-secondary)', fontWeight: 500, marginBottom: 8 }}>
            {t('calibration.onboarding.setupComplete')}
          </p>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              {/* eslint-disable-next-line i18next/no-literal-string */}
              <span style={{ color: 'var(--color-success)', fontSize: 12 }}>&#10003;</span>
              <span style={{ fontSize: 11, color: 'var(--color-text-secondary)' }}>
                {embeddingMode === 'keyword-only'
                  ? t('calibration.onboarding.summaryKeyword')
                  : t('calibration.onboarding.summaryAI')
                }
              </span>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              {/* eslint-disable-next-line i18next/no-literal-string */}
              <span style={{ color: 'var(--color-success)', fontSize: 12 }}>&#10003;</span>
              <span style={{ fontSize: 11, color: 'var(--color-text-secondary)' }}>
                {(setupCounts?.tech ?? 0) > 0
                  ? t('calibration.onboarding.summaryProjects', {
                      count: setupCounts!.tech,
                    })
                  : t('calibration.onboarding.summaryNoProjects')
                }
              </span>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              {/* eslint-disable-next-line i18next/no-literal-string */}
              <span style={{ color: 'var(--color-success)', fontSize: 12 }}>&#10003;</span>
              <span style={{ fontSize: 11, color: 'var(--color-text-secondary)' }}>
                {(setupCounts?.interests ?? 0) > 0
                  ? t('calibration.onboarding.summaryInterests', {
                      count: setupCounts!.interests,
                    })
                  : t('calibration.onboarding.summaryDefaultInterests')
                }
              </span>
            </div>
          </div>
        </div>
      )}

      {/* Trial hint */}
      <p className="mt-4 text-[10px] text-text-muted/60 text-center">
        {t('onboarding.trialHint')}
      </p>

      {/* Navigation */}
      <div className="flex justify-between mt-4">
        <button onClick={onBack} className="px-4 py-2 text-sm text-text-secondary hover:text-text-primary transition-colors">
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
