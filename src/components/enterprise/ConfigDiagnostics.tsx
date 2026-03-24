import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { cmd } from '../../lib/commands';

interface DiagnosticCheck {
  name: string;
  status: 'pass' | 'warn' | 'fail' | 'running';
  message: string;
}

const STATUS_STYLES: Record<string, { bg: string; text: string; icon: string }> = {
  pass: { bg: 'bg-success/10', text: 'text-success', icon: '\u2714' },
  warn: { bg: 'bg-[#F97316]/10', text: 'text-[#F97316]', icon: '\u26A0' },
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

    const addCheck = (name: string, status: 'pass' | 'warn' | 'fail', message: string) => {
      results.push({ name, status, message });
      setChecks([...results]);
    };

    // 1. License check
    setChecks([...results, { name: 'License', status: 'running', message: 'Checking...' }]);
    try {
      const isPaid = tier !== 'free';
      addCheck('License', isPaid ? 'pass' : 'warn',
        isPaid ? `Active: ${tier} tier` : 'Free tier — team features require Signal+ tier'
      );
    } catch {
      addCheck('License', 'fail', 'Could not verify license status');
    }

    // 2. AI Provider check
    setChecks([...results, { name: 'AI Provider', status: 'running', message: 'Checking...' }]);
    try {
      await cmd('test_llm_connection');
      addCheck('AI Provider', 'pass', 'LLM connection successful');
    } catch {
      addCheck('AI Provider', 'warn', 'LLM not configured or unreachable — offline mode active');
    }

    // 3. Embedding check
    setChecks([...results, { name: 'Embeddings', status: 'running', message: 'Checking...' }]);
    try {
      const ollamaStatus = await cmd('check_ollama_status', { baseUrl: null }).catch(() => null);
      if (ollamaStatus && ollamaStatus.operational) {
        addCheck('Embeddings', 'pass', 'Ollama running — local embeddings active');
      } else {
        addCheck('Embeddings', 'warn', 'Ollama not detected — using zero-vector fallback');
      }
    } catch {
      addCheck('Embeddings', 'warn', 'Embedding check failed');
    }

    // 4. Database check (use source health as proxy for DB connectivity)
    setChecks([...results, { name: 'Database', status: 'running', message: 'Checking...' }]);
    try {
      // If we can query sources, the DB is working
      await cmd('get_source_health_status');
      addCheck('Database', 'pass', 'SQLite connection OK');
    } catch {
      addCheck('Database', 'fail', 'Database connection failed');
    }

    // 5. Source health check
    setChecks([...results, { name: 'Sources', status: 'running', message: 'Checking...' }]);
    try {
      const sources = await cmd('get_source_health_status');
      const arr = sources;
      const healthy = arr.filter(s => s.status === 'healthy').length;
      const total = arr.length;
      if (total === 0) {
        addCheck('Sources', 'warn', 'No sources configured');
      } else if (healthy === total) {
        addCheck('Sources', 'pass', `All ${total} sources healthy`);
      } else {
        addCheck('Sources', 'warn', `${healthy}/${total} sources healthy`);
      }
    } catch {
      addCheck('Sources', 'warn', 'Could not check source health');
    }

    // 6. Team relay check
    setChecks([...results, { name: 'Team Relay', status: 'running', message: 'Checking...' }]);
    if (teamStatus?.enabled) {
      if (teamStatus.connected) {
        addCheck('Team Relay', 'pass',
          `Connected — ${teamStatus.member_count} members, ${teamStatus.pending_outbound} pending`
        );
      } else {
        addCheck('Team Relay', 'warn', 'Team configured but relay disconnected');
      }
    } else {
      addCheck('Team Relay', 'pass', 'Not configured (single-user mode)');
    }

    // 7. Webhook check (enterprise only)
    if (tier === 'enterprise') {
      setChecks([...results, { name: 'Webhooks', status: 'running', message: 'Checking...' }]);
      try {
        const webhooks = await cmd('list_webhooks_cmd');
        const wh = webhooks;
        const active = wh.filter(w => w.active).length;
        const failed = wh.filter(w => w.failure_count >= 5).length;
        if (wh.length === 0) {
          addCheck('Webhooks', 'pass', 'No webhooks configured');
        } else if (failed > 0) {
          addCheck('Webhooks', 'warn', `${active} active, ${failed} with circuit breaker open`);
        } else {
          addCheck('Webhooks', 'pass', `${active} active webhooks`);
        }
      } catch {
        addCheck('Webhooks', 'warn', 'Could not check webhook status');
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
            {t('enterprise.diagnostics.title', 'Configuration Diagnostics')}
          </h3>
          <p className="text-[10px] text-text-muted mt-0.5">
            {t('enterprise.diagnostics.description', 'Validate your setup and troubleshoot issues without contacting support.')}
          </p>
        </div>
        <button
          onClick={runDiagnostics}
          disabled={running}
          className="px-3 py-1.5 text-xs bg-success/15 text-success rounded hover:bg-success/25 transition-colors disabled:opacity-50"
        >
          {running ? t('enterprise.diagnostics.running', 'Running...') : t('enterprise.diagnostics.run', 'Run Diagnostics')}
        </button>
      </div>

      {/* Summary */}
      {checks.length > 0 && !running && (
        <div className="flex items-center gap-3">
          {passCount > 0 && (
            <span className="text-[10px] text-success">{passCount} passed</span>
          )}
          {warnCount > 0 && (
            <span className="text-[10px] text-[#F97316]">{warnCount} warnings</span>
          )}
          {failCount > 0 && (
            <span className="text-[10px] text-error">{failCount} failed</span>
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
            {t('enterprise.diagnostics.empty', 'Click "Run Diagnostics" to validate your configuration.')}
          </p>
        </div>
      )}

      {/* Troubleshooting Tips */}
      <div>
        <h4 className="text-xs font-medium text-text-secondary mb-2">
          {t('enterprise.diagnostics.tips', 'Quick Fixes')}
        </h4>
        <div className="space-y-1.5 text-[10px] text-text-muted">
          <div className="flex items-start gap-2">
            <span className="text-accent-gold mt-0.5">&#8226;</span>
            <span><strong className="text-text-secondary">No LLM:</strong> Add an API key in Settings &gt; General &gt; AI Provider</span>
          </div>
          <div className="flex items-start gap-2">
            <span className="text-accent-gold mt-0.5">&#8226;</span>
            <span><strong className="text-text-secondary">No embeddings:</strong> Install Ollama and run <code className="text-[#818CF8]">ollama pull nomic-embed-text</code></span>
          </div>
          <div className="flex items-start gap-2">
            <span className="text-accent-gold mt-0.5">&#8226;</span>
            <span><strong className="text-text-secondary">Relay disconnected:</strong> Check your network and verify the relay URL in Team settings</span>
          </div>
          <div className="flex items-start gap-2">
            <span className="text-accent-gold mt-0.5">&#8226;</span>
            <span><strong className="text-text-secondary">Source errors:</strong> Check API rate limits. Circuit breakers auto-recover after 30 minutes.</span>
          </div>
        </div>
      </div>
    </div>
  );
}
