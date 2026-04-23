// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { cmd } from '../../lib/commands';

interface DiagnosticCheck {
  key: string;
  name: string;
  status: 'pass' | 'warn' | 'fail' | 'running';
  message: string;
}

const STATUS_STYLES: Record<string, { bg: string; text: string; icon: string }> = {
  pass: { bg: 'bg-success/10', text: 'text-success', icon: '\u2714' },
  warn: { bg: 'bg-[var(--color-accent-action)]/10', text: 'text-[var(--color-accent-action)]', icon: '\u26A0' },
  fail: { bg: 'bg-error/10', text: 'text-error', icon: '\u2716' },
  running: { bg: 'bg-[#3B82F6]/10', text: 'text-[#3B82F6]', icon: '\u23F3' },
};

export function ConfigDiagnostics() {
  const { t } = useTranslation();
  const teamStatus = useAppStore(s => s.teamStatus);
  const tier = useAppStore(s => s.tier);

  const [checks, setChecks] = useState<DiagnosticCheck[]>([]);
  const [running, setRunning] = useState(false);

  const runDiagnostics = async () => {
    setRunning(true);
    const results: DiagnosticCheck[] = [];

    const addCheck = (key: string, name: string, status: 'pass' | 'warn' | 'fail', message: string) => {
      results.push({ key, name, status, message });
      setChecks([...results]);
    };

    // 1. License check
    setChecks([...results, { key: 'license', name: t('enterprise.diagnostics.check.license'), status: 'running', message: t('enterprise.diagnostics.checking') }]);
    try {
      const isPaid = tier !== 'free';
      addCheck('license', t('enterprise.diagnostics.check.license'), isPaid ? 'pass' : 'warn',
        isPaid ? t('enterprise.diagnostics.licenseActive', { tier }) : t('enterprise.diagnostics.licenseFree')
      );
    } catch {
      addCheck('license', t('enterprise.diagnostics.check.license'), 'fail', t('enterprise.diagnostics.licenseFail'));
    }

    // 2. AI Provider check
    setChecks([...results, { key: 'ai', name: t('enterprise.diagnostics.check.aiProvider'), status: 'running', message: t('enterprise.diagnostics.checking') }]);
    try {
      await cmd('test_llm_connection');
      addCheck('ai', t('enterprise.diagnostics.check.aiProvider'), 'pass', t('enterprise.diagnostics.llmOk'));
    } catch {
      addCheck('ai', t('enterprise.diagnostics.check.aiProvider'), 'warn', t('enterprise.diagnostics.llmWarn'));
    }

    // 3. Embedding check
    setChecks([...results, { key: 'embeddings', name: t('enterprise.diagnostics.check.embeddings'), status: 'running', message: t('enterprise.diagnostics.checking') }]);
    try {
      const ollamaStatus = await cmd('check_ollama_status', { baseUrl: null }).catch(() => null);
      if (ollamaStatus && ollamaStatus.operational) {
        addCheck('embeddings', t('enterprise.diagnostics.check.embeddings'), 'pass', t('enterprise.diagnostics.ollamaOk'));
      } else {
        addCheck('embeddings', t('enterprise.diagnostics.check.embeddings'), 'warn', t('enterprise.diagnostics.ollamaWarn'));
      }
    } catch {
      addCheck('embeddings', t('enterprise.diagnostics.check.embeddings'), 'warn', t('enterprise.diagnostics.embeddingFail'));
    }

    // 4. Database check (use source health as proxy for DB connectivity)
    setChecks([...results, { key: 'database', name: t('enterprise.diagnostics.check.database'), status: 'running', message: t('enterprise.diagnostics.checking') }]);
    try {
      // If we can query sources, the DB is working
      await cmd('get_source_health_status');
      addCheck('database', t('enterprise.diagnostics.check.database'), 'pass', t('enterprise.diagnostics.dbOk'));
    } catch {
      addCheck('database', t('enterprise.diagnostics.check.database'), 'fail', t('enterprise.diagnostics.dbFail'));
    }

    // 5. Source health check
    setChecks([...results, { key: 'sources', name: t('enterprise.diagnostics.check.sources'), status: 'running', message: t('enterprise.diagnostics.checking') }]);
    try {
      const sources = await cmd('get_source_health_status');
      const arr = sources;
      const healthy = arr.filter(s => s.status === 'healthy').length;
      const total = arr.length;
      if (total === 0) {
        addCheck('sources', t('enterprise.diagnostics.check.sources'), 'warn', t('enterprise.diagnostics.noSources'));
      } else if (healthy === total) {
        addCheck('sources', t('enterprise.diagnostics.check.sources'), 'pass', t('enterprise.diagnostics.allHealthy', { total }));
      } else {
        addCheck('sources', t('enterprise.diagnostics.check.sources'), 'warn', t('enterprise.diagnostics.someHealthy', { healthy, total }));
      }
    } catch {
      addCheck('sources', t('enterprise.diagnostics.check.sources'), 'warn', t('enterprise.diagnostics.sourceCheckFail'));
    }

    // 6. Team relay check
    setChecks([...results, { key: 'relay', name: t('enterprise.diagnostics.check.relay'), status: 'running', message: t('enterprise.diagnostics.checking') }]);
    if (teamStatus?.enabled) {
      if (teamStatus.connected) {
        addCheck('relay', t('enterprise.diagnostics.check.relay'), 'pass',
          t('enterprise.diagnostics.relayConnected', { members: teamStatus.member_count, pending: teamStatus.pending_outbound })
        );
      } else {
        addCheck('relay', t('enterprise.diagnostics.check.relay'), 'warn', t('enterprise.diagnostics.relayDisconnected'));
      }
    } else {
      addCheck('relay', t('enterprise.diagnostics.check.relay'), 'pass', t('enterprise.diagnostics.relaySingleUser'));
    }

    // 7. Webhook check (enterprise only)
    if (tier === 'enterprise') {
      setChecks([...results, { key: 'webhooks', name: t('enterprise.diagnostics.check.webhooks'), status: 'running', message: t('enterprise.diagnostics.checking') }]);
      try {
        const webhooks = await cmd('list_webhooks_cmd');
        const wh = webhooks;
        const active = wh.filter(w => w.active).length;
        const failed = wh.filter(w => w.failure_count >= 5).length;
        if (wh.length === 0) {
          addCheck('webhooks', t('enterprise.diagnostics.check.webhooks'), 'pass', t('enterprise.diagnostics.noWebhooks'));
        } else if (failed > 0) {
          addCheck('webhooks', t('enterprise.diagnostics.check.webhooks'), 'warn', t('enterprise.diagnostics.webhooksDegraded', { active, failed }));
        } else {
          addCheck('webhooks', t('enterprise.diagnostics.check.webhooks'), 'pass', t('enterprise.diagnostics.webhooksOk', { active }));
        }
      } catch {
        addCheck('webhooks', t('enterprise.diagnostics.check.webhooks'), 'warn', t('enterprise.diagnostics.webhookCheckFail'));
      }
    }

    setRunning(false);
  };

  const passCount = checks.filter(c => c.status === 'pass').length;
  const warnCount = checks.filter(c => c.status === 'warn').length;
  const failCount = checks.filter(c => c.status === 'fail').length;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-white">
            {t('enterprise.diagnostics.title')}
          </h3>
          <p className="text-[10px] text-text-muted mt-0.5">
            {t('enterprise.diagnostics.description')}
          </p>
        </div>
        <button
          onClick={runDiagnostics}
          disabled={running}
          className="px-3 py-1.5 text-xs bg-success/15 text-success rounded hover:bg-success/25 transition-colors disabled:opacity-50"
        >
          {running ? t('enterprise.diagnostics.running') : t('enterprise.diagnostics.run')}
        </button>
      </div>

      {/* Summary */}
      {checks.length > 0 && !running && (
        <div className="flex items-center gap-3">
          {passCount > 0 && (
            <span className="text-[10px] text-success">{passCount} {t('enterprise.diagnostics.passed')}</span>
          )}
          {warnCount > 0 && (
            <span className="text-[10px] text-[var(--color-accent-action)]">{warnCount} {t('enterprise.diagnostics.warnings')}</span>
          )}
          {failCount > 0 && (
            <span className="text-[10px] text-error">{failCount} {t('enterprise.diagnostics.failed')}</span>
          )}
        </div>
      )}

      {/* Check Results */}
      {checks.length > 0 && (
        <div className="space-y-1.5">
          {checks.map((check, i) => {
            const style = (STATUS_STYLES[check.status] ?? STATUS_STYLES.running)!;
            return (
              <div
                key={i}
                className={`flex items-center gap-3 px-3 py-2.5 rounded-lg border border-border/50 ${style.bg}`}
              >
                <span className={`text-sm ${style.text}`}>{style.icon}</span>
                <div className="flex-1 min-w-0">
                  <span className="text-xs text-white font-medium">{check.name}</span>
                  <p className={`text-[10px] ${style.text}`}>{check.message}</p>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Empty State */}
      {checks.length === 0 && (
        <div className="text-center py-6">
          <p className="text-xs text-text-muted">
            {t('enterprise.diagnostics.empty')}
          </p>
        </div>
      )}

      {/* Troubleshooting Tips — only shown for checks that need attention */}
      {(() => {
        const isFailing = (key: string) => checks.some(c => c.key === key && c.status !== 'pass');
        const tips: { key: string; label: string; content: React.ReactNode }[] = [
          { key: 'ai', label: 'No LLM:', content: <>Add an API key in Settings &gt; General &gt; AI Provider</> },
          { key: 'embeddings', label: 'No embeddings:', content: <>Install Ollama and run <code className="text-[#818CF8]">ollama pull nomic-embed-text</code></> },
          { key: 'relay', label: 'Relay disconnected:', content: 'Check your network and verify the relay URL in Team settings' },
          { key: 'sources', label: 'Source errors:', content: 'Check API rate limits. Circuit breakers auto-recover after 30 minutes.' },
        ];
        const activeTips = checks.length > 0 ? tips.filter(tip => isFailing(tip.key)) : tips;
        if (activeTips.length === 0) return null;
        return (
          <div>
            <h4 className="text-xs font-medium text-text-secondary mb-2">
              {t('enterprise.diagnostics.tips')}
            </h4>
            <div className="space-y-1.5 text-[10px] text-text-muted">
              {activeTips.map(tip => (
                <div key={tip.key} className="flex items-start gap-2">
                  <span className="text-accent-gold mt-0.5">&#8226;</span>
                  <span><strong className="text-text-secondary">{tip.label}</strong> {tip.content}</span>
                </div>
              ))}
            </div>
          </div>
        );
      })()}
    </div>
  );
}
