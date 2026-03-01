import { useState } from 'react';
import { cmd } from '../lib/commands';
import type { CalibrationResult } from '../types/calibration';

export function CalibrationView() {
  const [result, setResult] = useState<CalibrationResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runCalibration = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await cmd('run_calibration');
      setResult(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const gradeColor = (grade: string) => {
    if (grade.startsWith('A')) return '#22C55E';
    if (grade.startsWith('B')) return '#D4AF37';
    if (grade.startsWith('C')) return '#F59E0B';
    if (grade.startsWith('D')) return '#EF4444';
    return '#EF4444';
  };

  const priorityColor = (p: string) => {
    if (p === 'P0') return '#EF4444';
    if (p === 'P1') return '#F59E0B';
    return '#666666';
  };

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
          {loading ? 'Calibrating...' : result ? 'Re-calibrate' : 'Run Calibration'}
        </button>
      </div>

      {error && (
        <div style={{ padding: 12, background: '#1a0000', border: '1px solid #EF4444', borderRadius: 6, color: '#EF4444', fontSize: 13, marginBottom: 16 }}>
          {error}
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
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 12 }}>
                <MetricBox label="Accuracy" value={`${(result.aggregate_f1 * 100).toFixed(0)}%`} />
                <MetricBox label="Precision" value={`${(result.aggregate_precision * 100).toFixed(0)}%`} />
                <MetricBox label="Separation" value={result.mean_separation_gap.toFixed(2)} />
              </div>
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

          {/* Recommendations */}
          {result.recommendations.length > 0 && (
            <div style={{ background: '#141414', border: '1px solid #2A2A2A', borderRadius: 8, padding: 16 }}>
              <h3 style={{ margin: '0 0 12px', fontSize: 14, fontWeight: 600, color: '#FFFFFF' }}>
                Recommendations
              </h3>
              {result.recommendations.map((rec, i) => (
                <div key={i} style={{ padding: '8px 0', borderTop: i > 0 ? '1px solid #1F1F1F' : 'none' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                    <span style={{
                      fontSize: 10,
                      fontWeight: 700,
                      color: priorityColor(rec.priority),
                      fontFamily: 'JetBrains Mono, monospace',
                    }}>
                      {rec.priority}
                    </span>
                    <span style={{ fontSize: 13, fontWeight: 500, color: '#FFFFFF' }}>
                      {rec.title}
                    </span>
                  </div>
                  <p style={{ margin: '4px 0 0', fontSize: 12, color: '#A0A0A0', lineHeight: 1.4 }}>
                    {rec.description}
                  </p>
                  {rec.action && (
                    <code style={{
                      display: 'inline-block',
                      marginTop: 4,
                      padding: '2px 6px',
                      background: '#1F1F1F',
                      borderRadius: 3,
                      fontSize: 11,
                      color: '#D4AF37',
                      fontFamily: 'JetBrains Mono, monospace',
                    }}>
                      {rec.action}
                    </code>
                  )}
                </div>
              ))}
            </div>
          )}
        </>
      )}

      {!result && !loading && (
        <div style={{
          textAlign: 'center',
          padding: '60px 20px',
          color: '#666666',
          fontSize: 13,
        }}>
          <div style={{ fontSize: 32, marginBottom: 12, opacity: 0.5 }}>&#x2699;</div>
          Run calibration to see your scoring quality grade,
          rig capabilities, and what you can do to improve accuracy.
        </div>
      )}
    </div>
  );
}

function MetricBox({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ textAlign: 'center' }}>
      <div style={{ fontSize: 20, fontWeight: 600, color: '#FFFFFF', fontFamily: 'JetBrains Mono, monospace' }}>
        {value}
      </div>
      <div style={{ fontSize: 11, color: '#666666', marginTop: 2 }}>{label}</div>
    </div>
  );
}

function StatusRow({ label, ok, detail }: { label: string; ok: boolean; detail: string }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <span style={{ color: ok ? '#22C55E' : '#EF4444', fontSize: 14 }}>
        {ok ? '\u2713' : '\u2717'}
      </span>
      <span style={{ fontSize: 12, color: '#A0A0A0' }}>{label}:</span>
      <span style={{ fontSize: 12, color: '#FFFFFF', fontFamily: 'JetBrains Mono, monospace' }}>
        {detail}
      </span>
    </div>
  );
}
