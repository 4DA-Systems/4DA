// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';

/** Settings tabs a health issue can be fixed from. */
type FixTab = 'projects' | 'intelligence' | 'sources' | 'about';

interface DotIssue {
  severity: 'warning' | 'error';
  message: string;
  fixTab: FixTab;
}

/**
 * Where each capability's problem is fixed. Project Context ("No project
 * directories configured") is fixed by adding a folder in Projects; the model
 * capabilities live under Intelligence. Anything without a settings control
 * opens About, which carries the diagnostics.
 */
const CAPABILITY_FIX_TAB: Record<string, FixTab> = {
  ace_context: 'projects',
  embedding_search: 'intelligence',
  llm_reranking: 'intelligence',
  briefing_generation: 'intelligence',
  source_fetching: 'sources',
};

const STARTUP_FIX_TAB: Record<string, FixTab> = {
  embedding: 'intelligence',
  sources: 'sources',
};

/** Startup issues + live capability states, errors first. Exported for tests. */
export function collectHealthIssues(
  startup: Array<{ component: string; severity: 'warning' | 'error'; message: string }>,
  capabilities: Record<string, { state: string; reason?: string }> | null,
): DotIssue[] {
  const issues: DotIssue[] = startup.map(i => ({
    severity: i.severity,
    message: i.message,
    fixTab: STARTUP_FIX_TAB[i.component] ?? 'about',
  }));
  // Serde casing is lowercase: "full" | "degraded" | "unavailable".
  for (const [cap, s] of Object.entries(capabilities ?? {})) {
    if (s.state !== 'degraded' && s.state !== 'unavailable') continue;
    issues.push({
      severity: s.state === 'unavailable' ? 'error' : 'warning',
      message: s.reason ?? cap,
      fixTab: CAPABILITY_FIX_TAB[cap] ?? 'about',
    });
  }
  return issues.sort((a, b) => (a.severity === b.severity ? 0 : a.severity === 'error' ? -1 : 1));
}

/**
 * Persistent system health indicator — a small, still dot in the header.
 *
 * Amber: warnings (boot-time warning OR a runtime-degraded capability)
 * Red: errors (boot-time error OR a runtime-unavailable capability, e.g. a
 *      rejected API key surfaced by the LLM client)
 * Hidden: healthy, or the health check cannot run (never block the app)
 *
 * Fresh-profile E2E 2026-10-10: the "No project directories" warning stayed
 * after onboarding added projects, the dot pulsed for the whole session (the
 * last idle animation after #891), and clicking it opened Settings > General,
 * where nothing could fix it. Now: the backend reconciles Project Context on
 * every capability read, both signals are re-read when Settings closes (where
 * fixes happen), the dot never animates (a standing condition is not
 * activity), and a click opens the tab that fixes the top issue.
 */
export function SystemHealthDot() {
  const { t } = useTranslation();
  const loadStartupHealth = useAppStore(s => s.loadStartupHealth);
  const startupIssues = useAppStore(s => s.startupHealthIssues);
  const loadCapabilityStates = useAppStore(s => s.loadCapabilityStates);
  const capabilityStates = useAppStore(s => s.capabilityStates);
  const showSettings = useAppStore(s => s.showSettings);
  const setShowSettings = useAppStore(s => s.setShowSettings);
  const setSettingsInitialTab = useAppStore(s => s.setSettingsInitialTab);

  // Poll live capability states on mount and every 60s.
  useEffect(() => {
    void loadCapabilityStates();
    const id = setInterval(() => { void loadCapabilityStates(); }, 60_000);
    return () => clearInterval(id);
  }, [loadCapabilityStates]);

  useEffect(() => {
    if (startupIssues === null) void loadStartupHealth();
  }, [startupIssues, loadStartupHealth]);

  // Settings is where a warning gets fixed: re-check the moment it closes
  // instead of waiting up to a minute for the next poll.
  const settingsWasOpen = useRef(showSettings);
  useEffect(() => {
    if (settingsWasOpen.current && !showSettings) {
      void loadCapabilityStates();
      void loadStartupHealth();
    }
    settingsWasOpen.current = showSettings;
  }, [showSettings, loadCapabilityStates, loadStartupHealth]);

  const issues = useMemo(
    () => (startupIssues === null ? null : collectHealthIssues(startupIssues, capabilityStates)),
    [startupIssues, capabilityStates],
  );

  if (!issues || issues.length === 0) return null;

  const top = issues[0]!;
  const isError = top.severity === 'error';
  const title = t(isError ? 'health.dot.error' : 'health.dot.warning', {
    count: issues.length,
    detail: top.message,
  });

  return (
    <button
      type="button"
      onClick={() => {
        setSettingsInitialTab(top.fixTab);
        setShowSettings(true);
      }}
      className={`w-2 h-2 rounded-full ${isError ? 'bg-error' : 'bg-accent-gold'}`}
      title={title}
      aria-label={title}
      data-severity={top.severity}
      data-count={issues.length}
      data-fix-tab={top.fixTab}
    />
  );
}
