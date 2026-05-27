// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { useAppStore } from '../store';

/**
 * Persistent system health indicator — tiny colored dot in the header.
 *
 * Green: all systems healthy
 * Amber: warnings (degraded but functional)
 * Red: errors (some features unavailable)
 * Hidden: if health check fails or can't run (don't block the app)
 *
 * Click opens Settings (About tab has full diagnostics).
 */
export function SystemHealthDot({ onClick }: { onClick?: () => void }) {
  const [status, setStatus] = useState<'healthy' | 'warning' | 'error' | null>(null);
  const [issueCount, setIssueCount] = useState(0);
  const loadStartupHealth = useAppStore(s => s.loadStartupHealth);
  const startupIssues = useAppStore(s => s.startupHealthIssues);

  useEffect(() => {
    if (startupIssues === null) {
      void loadStartupHealth();
      return;
    }
    if (startupIssues.length === 0) {
      setStatus('healthy');
      return;
    }
    const hasErrors = startupIssues.some(i => i.severity === 'error');
    setStatus(hasErrors ? 'error' : 'warning');
    setIssueCount(startupIssues.length);
  }, [startupIssues, loadStartupHealth]);

  if (status === null || status === 'healthy') return null;

  const dotColor = status === 'error' ? 'bg-error' : 'bg-accent-gold';
  const title = status === 'error'
    ? `${issueCount} system error${issueCount > 1 ? 's' : ''} — click for diagnostics`
    : `${issueCount} system warning${issueCount > 1 ? 's' : ''} — click for diagnostics`;

  return (
    <button
      onClick={onClick}
      className={`w-2 h-2 rounded-full ${dotColor} animate-pulse`}
      title={title}
      aria-label={title}
    />
  );
}
