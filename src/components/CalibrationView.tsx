import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../lib/commands';
import { useAppStore } from '../store';
import { DimensionBar, StatusRow, RecommendationItem } from './calibration/CalibrationComponents';
import { gradeColor } from './calibration/calibration-utils';
import type { CalibrationResult, Recommendation } from '../types/calibration';
import type { PullProgress } from './calibration/calibration-utils';

export function CalibrationView() {
  const [result, setResult] = useState<CalibrationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [prevGrade, setPrevGrade] = useState<string | null>(null);
  const [pullProgress, setPullProgress] = useState<PullProgress | null>(null);
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  const [, setDetectingStack] = useState(false);
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
          await invoke('pull_ollama_model', {
            model: result?.rig_requirements.recommended_model || 'nomic-embed-text',
            baseUrl: null,
          });
        } catch (e) {
          setError(`Model pull failed: ${e instanceof Error ? e.message : String(e)}`);
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
        setDetectingStack(true);
        setActionInProgress('auto_detect_stacks');
        try {
          const detected = await invoke<Array<{ profile_id: string; confidence: number }>>('detect_stack_profiles');
          if (detected.length > 0) {
            const topIds = detected.slice(0, 3).map(d => d.profile_id);
            await invoke('set_selected_stacks', { profileIds: topIds });
            // Auto re-calibrate after stack detection
            await runCalibration();
          } else {
            // No auto-detection — open settings as fallback
            setShowSettings(true);
          }
        } catch (e) {
          setError(`Stack detection failed: ${e instanceof Error ? e.message : String(e)}`);
        } finally {
          setDetectingStack(false);
          setActionInProgress(null);
        }
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
    <div style={{ padding: '24px', maxWidth: 720 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 24 }}>
        <div>
          <h2 style={{ margin: 0, fontSize: 20, fontWeight: 600, color: '#FFFFFF' }}>
            Calibrate Your Rig
          </h2>
          <p style={{ margin: '4px 0 0', color: '#A0A0A0', fontSize: 13 }}>
            Honest scoring quality assessment for your setup
          </p>
        </div>
        <button
          onClick={runCalibration}
          disabled={loading}
          style={{
            padding: '8px 20px',
            background: loading ? '#2A2A2A' : '#FFFFFF',
            color: loading ? '#666666' : '#0A0A0A',
            border: 'none',
            borderRadius: 6,
            fontSize: 13,
            fontWeight: 600,
            cursor: loading ? 'not-allowed' : 'pointer',
            fontFamily: 'Inter, sans-serif',
          }}
        >
          {loading ? 'Calibrating...' : 'Re-calibrate'}
        </button>
      </div>

      {error && (
        <div style={{ padding: 12, background: '#1a0000', border: '1px solid #EF4444', borderRadius: 6, color: '#EF4444', fontSize: 13, marginBottom: 16 }}>
          {error}
        </div>
      )}

      {/* Grade improvement banner */}
      {gradeImproved && (
        <div style={{
          padding: 12,
          background: '#0a1a0a',
          border: '1px solid #22C55E',
          borderRadius: 6,
          marginBottom: 16,
          display: 'flex',
          alignItems: 'center',
          gap: 8,
        }}>
          <span style={{ fontSize: 18 }}>&#x2191;</span>
          <span style={{ fontSize: 13, color: '#22C55E', fontWeight: 600 }}>
            Grade improved: {prevGrade} &#x2192; {result.grade}
          </span>
        </div>
      )}

      {/* Pull progress bar */}
      {pullProgress && (
        <div style={{ marginBottom: 16 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
            <span style={{ fontSize: 11, color: '#A0A0A0' }}>
              Pulling {pullProgress.model}...
            </span>
            <span style={{ fontSize: 11, color: '#D4AF37', fontFamily: 'JetBrains Mono, monospace' }}>
              {pullProgress.done ? 'Done' : `${pullProgress.percent}%`}
            </span>
          </div>
          <div style={{ height: 4, background: '#2A2A2A', borderRadius: 2, overflow: 'hidden' }}>
            <div style={{
              height: '100%',
              width: `${pullProgress.done ? 100 : pullProgress.percent}%`,
              background: pullProgress.done ? '#22C55E' : '#D4AF37',
              borderRadius: 2,
              transition: 'width 0.3s ease',
            }} />
          </div>
          <div style={{ fontSize: 10, color: '#666666', marginTop: 2 }}>
            {pullProgress.status}
          </div>
        </div>
      )}

      {result && (
        <>
          {/* Grade Card */}
          <div style={{ display: 'flex', gap: 16, marginBottom: 20 }}>
            <div style={{
              flex: '0 0 120px',
              background: '#141414',
              border: '1px solid #2A2A2A',
              borderRadius: 8,
              padding: 20,
              textAlign: 'center',
            }}>
              <div style={{ fontSize: 48, fontWeight: 700, color: gradeColor(result.grade), fontFamily: 'JetBrains Mono, monospace' }}>
                {result.grade}
              </div>
              <div style={{ fontSize: 12, color: '#666666', marginTop: 4 }}>
                {result.grade_score}/100
              </div>
            </div>
            <div style={{ flex: 1, background: '#141414', border: '1px solid #2A2A2A', borderRadius: 8, padding: 16 }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <DimensionBar label="Infrastructure" score={result.infrastructure_score} />
                <DimensionBar label="Context" score={result.context_richness_score} />
                <DimensionBar label="Signal Coverage" score={result.signal_coverage_score} />
                <DimensionBar label="Discrimination" score={result.discrimination_score} />
              </div>
              {result.active_signal_axes.length > 0 && (
                <div style={{ display: 'flex', gap: 4, marginTop: 10, flexWrap: 'wrap' }}>
                  {result.active_signal_axes.map(axis => (
                    <span key={axis} style={{
                      padding: '2px 8px', background: '#1F1F1F', borderRadius: 10,
                      fontSize: 10, color: '#22C55E', fontFamily: 'JetBrains Mono, monospace',
                    }}>{axis}</span>
                  ))}
                </div>
              )}
            </div>
          </div>

          {/* Rig Requirements */}
          <div style={{ background: '#141414', border: '1px solid #2A2A2A', borderRadius: 8, padding: 16, marginBottom: 20 }}>
            <h3 style={{ margin: '0 0 12px', fontSize: 14, fontWeight: 600, color: '#FFFFFF' }}>
              Rig Status
            </h3>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
              <StatusRow
                label="Ollama"
                ok={result.rig_requirements.ollama_running}
                detail={result.rig_requirements.ollama_running ? 'Running' : 'Not detected'}
              />
              <StatusRow
                label="Embeddings"
                ok={result.rig_requirements.embedding_available}
                detail={result.rig_requirements.embedding_model || 'Not available'}
              />
              <StatusRow
                label="Grade A Capable"
                ok={result.rig_requirements.can_reach_grade_a}
                detail={result.rig_requirements.can_reach_grade_a ? 'Yes' : 'Needs setup'}
              />
              <StatusRow
                label="Recommended RAM"
                ok={true}
                detail={`${result.rig_requirements.estimated_ram_gb} GB`}
              />
            </div>

            {!result.rig_requirements.can_reach_grade_a && (
              <div style={{ marginTop: 12, padding: 12, background: '#1F1F1F', borderRadius: 6 }}>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#D4AF37', marginBottom: 6 }}>
                  To reach Grade A:
                </div>
                {result.rig_requirements.grade_a_requirements.map((req, i) => (
                  <div key={i} style={{ fontSize: 12, color: '#A0A0A0', padding: '2px 0', paddingLeft: 12 }}>
                    {req}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Recommendations with action buttons */}
          {result.recommendations.length > 0 && (
            <div style={{ background: '#141414', border: '1px solid #2A2A2A', borderRadius: 8, padding: 16 }}>
              <h3 style={{ margin: '0 0 4px', fontSize: 14, fontWeight: 600, color: '#FFFFFF' }}>
                {result.grade.startsWith('A')
                  ? 'Maintaining Grade A'
                  : `Upgrade Path to Grade A`}
              </h3>
              <p style={{ margin: '0 0 12px', fontSize: 11, color: '#666666' }}>
                Fix these to improve your grade. Actions auto-recalibrate when complete.
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
            <div style={{
              background: '#0a1a0a',
              border: '1px solid #22C55E',
              borderRadius: 8,
              padding: 20,
              textAlign: 'center',
            }}>
              <div style={{ fontSize: 24, marginBottom: 8 }}>&#x2713;</div>
              <div style={{ fontSize: 14, fontWeight: 600, color: '#22C55E' }}>
                Your rig is fully calibrated
              </div>
              <div style={{ fontSize: 12, color: '#A0A0A0', marginTop: 4 }}>
                All scoring systems are operating at maximum accuracy for your setup.
              </div>
            </div>
          )}
        </>
      )}

      {!result && loading && (
        <div style={{
          textAlign: 'center',
          padding: '60px 20px',
          color: '#A0A0A0',
          fontSize: 13,
        }}>
          Analyzing your rig and scoring accuracy...
        </div>
      )}
    </div>
  );
}
