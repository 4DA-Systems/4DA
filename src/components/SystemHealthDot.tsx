// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect } from 'react';
import { useAppStore } from '../store';

/**
 * Persistent system health indicator — tiny colored dot in the header.
 *
 * Green: all systems healthy
 * Amber: warnings (boot-time warning OR a runtime-degraded capability)
 * Red: errors (boot-time error OR a runtime-unavailable capability, e.g. a
 *      rejected API key surfaced by the LLM client)
 * Hidden: if health check fails or can't run (don't block the app)
 *
 * Merges two signals:
 * - boot-cached startup issues (one-shot, loaded on mount)
 * - live capability states (polled every 60s) so a runtime 401 on a stored
 *   API key shows up without a restart.
 *
 * Click opens Settings (About tab has full diagnostics).
 */
export function SystemHealthDot({ onClick }: { onClick?: () => void }) {
  const [status, setStatus] = useState<'healthy' | 'warning' | 'error' | null>(null);
  const [issueCount, setIssueCount] = useState(0);
  const loadStartupHealth = useAppStore(s => s.loadStartupHealth);
  const startupIssues = useAppStore(s => s.startupHealthIssues);
  const loadCapabilityStates = useAppStore(s => s.loadCapabilityStates);
  const capabilityStates = useAppStore(s => s.capabilityStates);

  // Poll live capability states on mount and every 60s.
  useEffect(() => {
    void loadCapabilityStates();
    const id = setInterval(() => { void loadCapabilityStates(); }, 60_000);
    return () => clearInterval(id);
  }, [loadCapabilityStates]);

  useEffect(() => {
    // Wait for the one-shot startup load before deciding anything.
    if (startupIssues === null) {
      void loadStartupHealth();
      return;
    }

    const startupErrors = startupIssues.filter(i => i.severity === 'error').length;
    const startupWarnings = startupIssues.length - startupErrors;

    // Serde casing is lowercase: "full" | "degraded" | "unavailable".
    const caps = capabilityStates ? Object.values(capabilityStates) : [];
    const capUnavailable = caps.filter(c => c.state === 'unavailable').length;
    const capDegraded = caps.filter(c => c.state === 'degraded').length;

    const errorCount = startupErrors + capUnavailable;
    const warningCount = startupWarnings + capDegraded;
    const total = errorCount + warningCount;

    if (total === 0) {
      setStatus('healthy');
      setIssueCount(0);
      return;
    }
    setStatus(errorCount > 0 ? 'error' : 'warning');
    setIssueCount(total);
  }, [startupIssues, loadStartupHealth, capabilityStates]);

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
