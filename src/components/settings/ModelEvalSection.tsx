// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

type Verdict = 'Pass' | 'PassWithWarnings' | 'Fail';

interface EvalResult {
  model_tag: string;
  provider: string;
  total_fixtures: number;
  passed: number;
  failed: number;
  critical_violations: number;
  verdict: Verdict;
  reports: Array<{
    fixture_name: string;
    passed: boolean;
    violations: Array<{ pattern: string; reason: string; severity: string; matched_text: string }>;
    missing_required: string[];
    duration_ms: number;
  }>;
}

export function ModelEvalSection() {
  const { t } = useTranslation();
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<EvalResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleRun = async () => {
    setRunning(true);
    setError(null);
    setResult(null);
    try {
      const summary = await cmd('run_model_eval');
      setResult(summary as unknown as EvalResult);
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  };

  const verdictStyle: Record<Verdict, { bg: string; text: string; label: string }> = {
    Pass: { bg: 'bg-green-500/15', text: 'text-green-400', label: t('settings.ai.evalPass', 'Pass') },
    PassWithWarnings: { bg: 'bg-amber-500/15', text: 'text-amber-400', label: t('settings.ai.evalWarnings', 'Pass with warnings') },
    Fail: { bg: 'bg-red-500/15', text: 'text-red-400', label: t('settings.ai.evalFail', 'Fail') },
  };

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <div className="flex items-center justify-between mb-2">
        <div>
          <h4 className="text-xs font-medium text-text-primary">{t('settings.ai.evalTitle', 'Model Quality Check')}</h4>
          <p className="text-[10px] text-text-muted mt-0.5">
            {t('settings.ai.evalDescription', 'Test your configured model against hallucination and accuracy fixtures')}
          </p>
        </div>
        <button
          type="button"
          onClick={() => void handleRun()}
          disabled={running}
          className="text-xs px-3 py-1.5 bg-orange-500/20 text-orange-300 rounded-lg hover:bg-orange-500/30 transition-colors font-medium disabled:opacity-40 flex items-center gap-1.5"
        >
          {running && <span className="w-3 h-3 border border-orange-400 border-t-transparent rounded-full animate-spin" />}
          {running ? t('settings.ai.evalRunning', 'Running...') : t('settings.ai.evalRun', 'Verify Model')}
        </button>
      </div>

      {error && (
        <div className="mt-2 p-2 bg-red-900/15 border border-red-500/30 rounded text-xs text-red-400">
          {error}
        </div>
      )}

      {result && (
        <div className="mt-3 space-y-2">
          <div className={`flex items-center justify-between p-2.5 rounded-lg border ${verdictStyle[result.verdict].bg} border-text-primary/5`}>
            <div className="flex items-center gap-2">
              <span className={`text-sm font-semibold ${verdictStyle[result.verdict].text}`}>
                {verdictStyle[result.verdict].label}
              </span>
              <span className="text-[10px] text-text-muted">
                {result.passed}/{result.total_fixtures} {t('settings.ai.evalFixtures', 'fixtures passed')}
              </span>
            </div>
            {result.critical_violations > 0 && (
              <span className="text-[10px] text-red-400 font-medium">
                {result.critical_violations} {t('settings.ai.evalCritical', 'critical')}
              </span>
            )}
          </div>

          {result.reports.filter(r => !r.passed).map((report) => (
            <div key={report.fixture_name} className="p-2 bg-bg-secondary rounded border border-border">
              <div className="flex items-center justify-between">
                <span className="text-xs text-red-400 font-medium">{report.fixture_name.replace(/_/g, ' ')}</span>
                {/* eslint-disable-next-line i18next/no-literal-string */}
                <span className="text-[10px] text-text-muted">{report.duration_ms}ms</span>
              </div>
              {report.violations.length > 0 && (
                <ul className="mt-1 space-y-0.5">
                  {report.violations.map((v, i) => (
                    <li key={i} className="text-[10px] text-text-muted">
                      <span className={v.severity === 'Critical' ? 'text-red-400' : 'text-amber-400'}>
                        {v.pattern}
                      </span>
                      {' — '}{v.reason}
                    </li>
                  ))}
                </ul>
              )}
              {report.missing_required.length > 0 && (
                <p className="mt-1 text-[10px] text-text-muted">
                  {t('settings.ai.evalMissing', 'Missing')}: {report.missing_required.join(', ')}
                </p>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
