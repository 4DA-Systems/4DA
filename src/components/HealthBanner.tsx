import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { useAppStore } from '../store';

interface HealthIssue {
  component: string;
  severity: 'warning' | 'error';
  message: string;
}

const FIX_HINTS: Record<string, string> = {
  embedding: 'Run "ollama pull nomic-embed-text" in your terminal, or add an API key in Settings.',
  database: 'Try restarting the app. If the issue persists, check file permissions on the data/ folder.',
  settings: 'Your settings file may be corrupted. Delete data/settings.json and restart (you\'ll need to re-enter your API keys).',
  sources: 'Go to Settings > Sources and ensure at least one source is enabled.',
  disk: 'Check that the app has write permissions to its data directory.',
};

/**
 * Dismissible health warning banner shown at the top of the app.
 *
 * Calls get_startup_health on mount. If issues are found, displays
 * a collapsible banner with severity-colored indicators and actionable
 * fix instructions. Users can dismiss it, and it won't re-show until
 * next app launch.
 */
export function HealthBanner() {
  const { t } = useTranslation();
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const [issues, setIssues] = useState<HealthIssue[]>([]);
  const [dismissed, setDismissed] = useState(false);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    cmd('get_startup_health')
      .then((result) => {
        if (result && result.length > 0) {
          setIssues(result);
        }
      })
      .catch(() => {
        // Health check itself failed — don't block the app
      });
  }, []);

  const handleDismiss = useCallback(() => {
    setDismissed(true);
  }, []);

  if (dismissed || issues.length === 0) return null;

  const hasErrors = issues.some(i => i.severity === 'error');
  const borderColor = hasErrors ? 'border-error/40' : 'border-accent-gold/40';
  const bgColor = hasErrors ? 'bg-error/5' : 'bg-accent-gold/5';
  const iconColor = hasErrors ? 'text-error' : 'text-accent-gold';

  return (
    <div className={`mx-4 mt-2 mb-1 ${bgColor} ${borderColor} border rounded-lg overflow-hidden`}>
      {/* Summary bar */}
      <div className="flex items-center justify-between px-3 py-2">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-2 text-start flex-1"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className={iconColor}>
            {hasErrors ? (
              <><circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" /></>
            ) : (
              <><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" /><line x1="12" y1="9" x2="12" y2="13" /><line x1="12" y1="17" x2="12.01" y2="17" /></>
            )}
          </svg>
          <span className={`text-xs font-medium ${hasErrors ? 'text-error' : 'text-accent-gold'}`}>
            {issues.length === 1
              ? issues[0]!.message
              : t('health.issueCount', { count: issues.length, defaultValue: '{{count}} system issues detected' })
            }
          </span>
          {issues.length > 1 && (
            <span className={`text-[10px] text-text-muted transition-transform ${expanded ? 'rotate-180' : ''}`}>
              &#9660;
            </span>
          )}
        </button>
        <button
          onClick={handleDismiss}
          className="text-text-muted hover:text-text-secondary text-xs px-1"
          aria-label={t('action.dismiss', 'Dismiss')}
        >
          &#10005;
        </button>
      </div>

      {/* Expanded details */}
      {expanded && issues.length > 1 && (
        <div className="px-3 pb-2 space-y-1.5 border-t border-border/30">
          {issues.map((issue, i) => (
            <div key={i} className="flex items-start gap-2 pt-1.5">
              <div className={`w-1.5 h-1.5 rounded-full mt-1.5 flex-shrink-0 ${
                issue.severity === 'error' ? 'bg-error' : 'bg-accent-gold'
              }`} />
              <div className="min-w-0">
                <span className="text-xs text-text-secondary">{issue.message}</span>
                {FIX_HINTS[issue.component] && (
                  <p className="text-[10px] text-text-muted mt-0.5">
                    {FIX_HINTS[issue.component]}
                    {(issue.component === 'embedding' || issue.component === 'sources' || issue.component === 'settings') && (
                      <button
                        onClick={() => setShowSettings(true)}
                        className="ml-1 text-amber-400 hover:text-amber-300 underline transition-colors"
                      >
                        {t('health.openSettings', 'Open Settings')}
                      </button>
                    )}
                  </p>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
