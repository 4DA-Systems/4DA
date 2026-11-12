// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * When the app may start an analysis on its own.
 *
 * An analysis fetches from the network. Two things went wrong before
 * (audit 2026-11-11):
 * - The main view mounts, hidden, behind the onboarding wizard. Its warm-up
 *   state started an analysis 3 s after mount, so 4DA fetched sources before
 *   the user had finished (or consented in) onboarding.
 * - The warm-up state re-ran the analysis 3 s after EVERY mount (a 15 s
 *   cooldown aside), so an offline first run looped on "Start Intelligence"
 *   each time the user came back to the Brief tab.
 *
 * Now: nothing starts while onboarding is open or undecided, and the app
 * starts at most one analysis of its own per session. The user's own
 * "Start Intelligence" / Analyze buttons are never gated.
 */
import { useAppStore } from '../store';

const SESSION_KEY = '4da-auto-analysis-started';

interface GateState {
  /** Settings were read, so whether onboarding is complete is known. A
   *  timed-out settings load lifts the splash but never sets this. */
  onboardingKnown: boolean;
  showOnboarding: boolean;
}

/** Settings were read and onboarding is not open: the user is in the app. */
export function onboardingSettled(s: GateState): boolean {
  return s.onboardingKnown && !s.showOnboarding;
}

/** True once the app started an analysis of its own in this session. */
export function autoAnalysisStartedThisSession(): boolean {
  try {
    return window.sessionStorage.getItem(SESSION_KEY) !== null;
  } catch {
    return false;
  }
}

export function markAutoAnalysisStarted(): void {
  try {
    window.sessionStorage.setItem(SESSION_KEY, String(Date.now()));
  } catch { /* storage unavailable: the in-memory guards still hold */ }
}

/**
 * Resolves once the onboarding decision is made: `'complete'` when the user
 * is in the app, `'onboarding'` when the wizard is (or was) shown — then the
 * first-run flow starts its own analysis after the wizard and callers must
 * not start another.
 */
export function onboardingDecision(): Promise<'complete' | 'onboarding'> {
  const decide = (s: GateState): 'complete' | 'onboarding' | null => {
    if (s.showOnboarding) return 'onboarding';
    return s.onboardingKnown ? 'complete' : null;
  };
  const now = decide(useAppStore.getState());
  if (now) return Promise.resolve(now);
  return new Promise((resolve) => {
    const unsubscribe = useAppStore.subscribe((s) => {
      const d = decide(s);
      if (!d) return;
      unsubscribe();
      resolve(d);
    });
  });
}
