import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

interface ExampleSignal {
  title: string;
  source: string;
  score: number;
}

const stackSignals: Record<string, ExampleSignal[]> = {
  react: [
    { title: 'React 20 Concurrent Features: What Changed for Your Components', source: 'Hacker News', score: 0.91 },
    { title: 'Next.js Security Advisory: Middleware Bypass in v14.x', source: 'GitHub Advisory', score: 0.88 },
    { title: 'Vite 7 Released: Faster HMR and Native TypeScript Support', source: 'Reddit', score: 0.85 },
    { title: 'React Server Components: Production Patterns from Meta', source: 'Hacker News', score: 0.82 },
  ],
  rust: [
    { title: 'Tokio 2.0: Breaking Changes to Async Runtime Configuration', source: 'Reddit', score: 0.93 },
    { title: 'Cargo Workspace Improvements: Shared Dependency Resolution', source: 'Hacker News', score: 0.87 },
    { title: 'WASM Component Model: Rust Toolchain Updates', source: 'GitHub', score: 0.84 },
    { title: 'Rust 1.85 Stabilizes Async Closures and Let Chains', source: 'Reddit', score: 0.81 },
  ],
  python: [
    { title: 'Python 3.13 Released: Free-Threaded Mode Now Default', source: 'Hacker News', score: 0.92 },
    { title: 'FastAPI 1.0: Stable Release After 5 Years', source: 'Reddit', score: 0.86 },
    { title: 'pip Security Advisory: Dependency Confusion in Private Indexes', source: 'GitHub Advisory', score: 0.83 },
  ],
  default: [
    { title: 'GitHub Copilot Workspace: AI-Powered Development Environment', source: 'Hacker News', score: 0.89 },
    { title: 'VS Code March 2026: Built-in Terminal AI and Profile Sync', source: 'Reddit', score: 0.85 },
    { title: 'OWASP Top 10 Updated: Supply Chain Attacks Now #2', source: 'Hacker News', score: 0.82 },
  ],
};

function resolveStack(detected: string[]): string {
  const lower = detected.map(s => s.toLowerCase());
  if (lower.some(s => s.includes('react') || s.includes('next') || s.includes('tsx'))) return 'react';
  if (lower.some(s => s.includes('rust') || s.includes('cargo'))) return 'rust';
  if (lower.some(s => s.includes('python') || s.includes('pip') || s.includes('django'))) return 'python';
  return 'default';
}

function stackLabel(key: string): string {
  const labels: Record<string, string> = { react: 'React', rust: 'Rust', python: 'Python', default: '' };
  return labels[key] || '';
}

interface SmartEmptyStateProps {
  detectedStack: string[];
}

export function SmartEmptyState({ detectedStack }: SmartEmptyStateProps) {
  const { t } = useTranslation();
  const key = useMemo(() => resolveStack(detectedStack), [detectedStack]);
  const signals = (stackSignals[key] ?? stackSignals.default)!;
  const label = stackLabel(key);

  return (
    <div className="bg-bg-primary rounded-lg px-6 py-8" aria-label="Example signals — real data will appear after your first analysis">
      <p className="text-sm text-text-secondary text-center mb-6">
        {label
          ? t('empty.whileAnalysisRunsStack', { stack: label })
          : t('empty.whileAnalysisRuns')
        }
      </p>

      <div className="text-xs text-amber-400/80 bg-amber-500/10 rounded px-3 py-1.5 mb-3">
        {t('empty.exampleBanner', 'These are example signals based on your detected stack. Run an analysis to see real results.')}
      </div>

      <div className="space-y-3">
        {signals.map(signal => (
          <div key={signal.title} className="bg-bg-tertiary rounded-lg border border-border p-4 flex items-start gap-3">
            <div className="flex flex-col items-center gap-1 flex-shrink-0">
              <span className="text-[10px] px-1.5 py-0.5 rounded font-medium bg-blue-500/10 text-blue-400">
                {signal.source}
              </span>
              <span className="text-xs font-mono font-medium text-text-secondary">
                {signal.score.toFixed(2)}
              </span>
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm text-white font-medium">{signal.title}</p>
            </div>
            <span
              className="text-[10px] px-2 py-1 rounded-md bg-amber-500/15 text-amber-400 border border-amber-500/30 font-medium uppercase tracking-wider flex-shrink-0"
              aria-label="This is an example signal, not real data"
            >
              {t('empty.example')}
            </span>
          </div>
        ))}
      </div>

      <p className="text-xs text-text-muted text-center mt-6">
        {t('empty.realSignalsArriving')}
      </p>
    </div>
  );
}
