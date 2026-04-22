// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useAppStore } from '../store';

const STORAGE_KEY = '4da_first_cve_shown';

interface FirstCveCardProps {
  cveId: string;
  packageName: string;
  severity: string;
  projectCount: number;
  minutesSincePublication: number;
}

const severityColors: Record<string, string> = {
  critical: 'border-red-500/40 bg-red-500/5',
  high: 'border-red-500/30 bg-red-500/5',
  medium: 'border-amber-500/30 bg-amber-500/5',
  low: 'border-yellow-500/20 bg-yellow-500/5',
};

const severityBadge: Record<string, string> = {
  critical: 'bg-red-500/20 text-red-400',
  high: 'bg-red-500/15 text-red-400',
  medium: 'bg-amber-500/15 text-amber-400',
  low: 'bg-yellow-500/15 text-yellow-400',
};

export function FirstCveCard({ cveId, packageName, severity, projectCount, minutesSincePublication }: FirstCveCardProps) {
  const setActiveView = useAppStore(s => s.setActiveView);
  const [visible, setVisible] = useState(() => {
    try { return localStorage.getItem(STORAGE_KEY) !== 'true'; } catch { return true; }
  });

  if (!visible) return null;

  const dismiss = () => {
    try { localStorage.setItem(STORAGE_KEY, 'true'); } catch { /* noop */ }
    setVisible(false);
  };

  const colors = severityColors[severity] || severityColors.medium;
  const badge = severityBadge[severity] || severityBadge.medium;

  return (
    <div className={`rounded-lg border ${colors} p-5`}>
      <div className="flex items-center gap-2 mb-3">
        <svg className="w-5 h-5 text-red-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
        </svg>
        <h3 className="text-sm font-semibold text-white">4DA Protected You</h3>
        <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium uppercase ${badge}`}>{severity}</span>
      </div>
      <p className="text-sm text-text-secondary mb-1">
        {cveId} affects <span className="text-white font-medium">{packageName}</span> in {projectCount} of your projects
      </p>
      <p className="text-xs text-text-muted mb-4">
        Detected {minutesSincePublication} minutes after publication
      </p>
      <div className="flex items-center gap-3">
        <button
          onClick={() => { setActiveView('results'); }}
          className="px-4 py-2 text-xs font-medium bg-red-500/10 text-red-400 border border-red-500/20 rounded-lg hover:bg-red-500/20 transition-colors"
        >
          View Details
        </button>
        <button
          onClick={dismiss}
          className="px-4 py-2 text-xs font-medium text-text-muted hover:text-text-secondary transition-colors"
        >
          Got it
        </button>
      </div>
    </div>
  );
}
