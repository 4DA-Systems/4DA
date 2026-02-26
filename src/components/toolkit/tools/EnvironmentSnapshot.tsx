import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

type EnvSnapshot = {
  os: string;
  os_version: string;
  hostname: string;
  git_branch: string | null;
  git_status: string | null;
  git_recent_commits: string[];
  node_version: string | null;
  pnpm_version: string | null;
  npm_version: string | null;
  rust_version: string | null;
  python_version: string | null;
  ports: [];
};

function formatTimestamp(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

function buildMarkdown(snapshot: EnvSnapshot, timestamp: string): string {
  const lines: string[] = [
    '## Environment Snapshot',
    `**Date:** ${timestamp}`,
    `**OS:** ${snapshot.os} (${snapshot.os_version})`,
    `**Host:** ${snapshot.hostname}`,
  ];

  if (snapshot.git_branch || snapshot.git_status) {
    lines.push('', '### Git');
    if (snapshot.git_branch) lines.push(`- Branch: \`${snapshot.git_branch}\``);
    if (snapshot.git_status) lines.push(`- Status:\n\`\`\`\n${snapshot.git_status}\n\`\`\``);
    if (snapshot.git_recent_commits.length > 0) {
      lines.push('- Recent commits:');
      for (const commit of snapshot.git_recent_commits) {
        lines.push(`  - ${commit}`);
      }
    }
  }

  const runtimes: [string, string | null][] = [
    ['Node', snapshot.node_version],
    ['pnpm', snapshot.pnpm_version],
    ['npm', snapshot.npm_version],
    ['Rust', snapshot.rust_version],
    ['Python', snapshot.python_version],
  ];
  const available = runtimes.filter(([, v]) => v != null);
  if (available.length > 0) {
    lines.push('', '### Runtimes');
    for (const [name, version] of available) {
      lines.push(`- ${name}: ${version}`);
    }
  }

  return lines.join('\n');
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="text-xs font-medium text-[#A0A0A0] uppercase tracking-wider mb-2">
      {children}
    </h3>
  );
}

function InfoRow({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex items-center justify-between py-1.5 border-b border-[#2A2A2A] last:border-0">
      <span className="text-xs text-[#A0A0A0]">{label}</span>
      <span className={`text-xs text-white ${mono ? 'font-mono' : ''}`}>{value}</span>
    </div>
  );
}

export default function EnvironmentSnapshot() {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<EnvSnapshot | null>(null);
  const [timestamp, setTimestamp] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const capture = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<EnvSnapshot>('toolkit_env_snapshot', { workingDir: null });
      setSnapshot(result);
      setTimestamp(formatTimestamp(new Date()));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setSnapshot(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const copyMarkdown = useCallback(async () => {
    if (!snapshot) return;
    const md = buildMarkdown(snapshot, timestamp);
    try {
      await navigator.clipboard.writeText(md);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setError('Failed to copy to clipboard');
    }
  }, [snapshot, timestamp]);

  const runtimes: [string, string | null][] = snapshot
    ? [
        ['Node', snapshot.node_version],
        ['pnpm', snapshot.pnpm_version],
        ['npm', snapshot.npm_version],
        ['Rust', snapshot.rust_version],
        ['Python', snapshot.python_version],
      ]
    : [];
  const availableRuntimes = runtimes.filter(([, v]) => v != null) as [string, string][];

  return (
    <div className="space-y-4">
      {/* Action bar */}
      <div className="flex items-center gap-3">
        <button
          onClick={capture}
          disabled={loading}
          className="flex items-center gap-2 px-4 py-2 text-xs font-medium bg-white text-[#0A0A0A] rounded-lg hover:bg-white/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {loading ? (
            <>
              <div className="w-3.5 h-3.5 border-2 border-[#0A0A0A]/30 border-t-[#0A0A0A] rounded-full animate-spin" />
              {t('toolkit.envSnapshot.capturing')}
            </>
          ) : (
            <>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10" />
                <polyline points="12 6 12 12 16 14" />
              </svg>
              {t('toolkit.envSnapshot.captureSnapshot')}
            </>
          )}
        </button>

        {snapshot && (
          <button
            onClick={copyMarkdown}
            className="flex items-center gap-2 px-3 py-2 text-xs text-[#A0A0A0] bg-[#141414] border border-[#2A2A2A] rounded-lg hover:text-white hover:border-white/20 transition-all"
          >
            {copied ? (
              <>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#22C55E" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="20 6 9 17 4 12" />
                </svg>
                <span className="text-[#22C55E]">{t('action.copied')}</span>
              </>
            ) : (
              <>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
                </svg>
                {t('toolkit.envSnapshot.copyAsMarkdown')}
              </>
            )}
          </button>
        )}

        {timestamp && (
          <span className="text-[10px] text-[#666] ml-auto font-mono">
            {timestamp}
          </span>
        )}
      </div>

      {/* Error */}
      {error && (
        <div className="px-4 py-3 bg-[#EF4444]/10 border border-[#EF4444]/30 rounded-lg">
          <p className="text-xs text-[#EF4444]">{error}</p>
        </div>
      )}

      {/* Empty state */}
      {!snapshot && !loading && !error && (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="#666" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mb-3">
            <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
            <line x1="8" y1="21" x2="16" y2="21" />
            <line x1="12" y1="17" x2="12" y2="21" />
          </svg>
          <p className="text-sm text-[#A0A0A0] mb-1">{t('toolkit.envSnapshot.empty')}</p>
          <p className="text-xs text-[#666]">
            {t('toolkit.envSnapshot.emptyHint')}
          </p>
        </div>
      )}

      {/* Snapshot results */}
      {snapshot && (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          {/* System section */}
          <div className="bg-[#141414] border border-[#2A2A2A] rounded-lg p-4">
            <SectionHeader>{t('toolkit.envSnapshot.system')}</SectionHeader>
            <InfoRow label="OS" value={snapshot.os} />
            <InfoRow label="Version" value={snapshot.os_version} mono />
            <InfoRow label="Hostname" value={snapshot.hostname} mono />
          </div>

          {/* Runtimes section */}
          {availableRuntimes.length > 0 && (
            <div className="bg-[#141414] border border-[#2A2A2A] rounded-lg p-4">
              <SectionHeader>{t('toolkit.envSnapshot.runtimeVersions')}</SectionHeader>
              {availableRuntimes.map(([name, version]) => (
                <InfoRow key={name} label={name} value={version} mono />
              ))}
            </div>
          )}

          {/* Git section — full width */}
          {(snapshot.git_branch || snapshot.git_status) && (
            <div className="bg-[#141414] border border-[#2A2A2A] rounded-lg p-4 lg:col-span-2">
              <SectionHeader>{t('toolkit.envSnapshot.git')}</SectionHeader>

              {snapshot.git_branch && (
                <div className="flex items-center gap-2 mb-3">
                  <span className="text-xs text-[#A0A0A0]">Branch:</span>
                  <span className="inline-flex items-center px-2 py-0.5 text-xs font-mono text-white bg-[#1F1F1F] border border-[#2A2A2A] rounded">
                    {snapshot.git_branch}
                  </span>
                </div>
              )}

              {snapshot.git_status && (
                <div className="mb-3">
                  <p className="text-xs text-[#A0A0A0] mb-1.5">Status:</p>
                  <pre className="text-xs font-mono text-[#A0A0A0] bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg p-3 overflow-auto max-h-48 whitespace-pre-wrap">
                    {snapshot.git_status}
                  </pre>
                </div>
              )}

              {snapshot.git_recent_commits.length > 0 && (
                <div>
                  <p className="text-xs text-[#A0A0A0] mb-1.5">{t('toolkit.envSnapshot.recentCommits')}:</p>
                  <div className="space-y-1">
                    {snapshot.git_recent_commits.map((commit, i) => (
                      <div
                        key={i}
                        className="text-xs font-mono text-[#A0A0A0] bg-[#0A0A0A] border border-[#2A2A2A] rounded px-3 py-1.5 truncate"
                        title={commit}
                      >
                        {commit}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
