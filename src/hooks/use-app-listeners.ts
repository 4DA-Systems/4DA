// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect } from 'react';
import { useAppStore } from '../store';
import type { ToastType } from '../store/types';
import { cmd } from '../lib/commands';
import { isVictauriDogfoodMode } from '../lib/startup-runtime';
import { safeListen } from '../lib/tauri-events';

interface AppListenersConfig {
  addToast: (type: ToastType, message: string) => void;
  setEmbeddingStatus: (status: 'active' | 'degraded' | 'unavailable') => void;
  setShowFramework: (show: boolean) => void;
  setShowComparison: (show: boolean) => void;
  setState: (fn: (s: ReturnType<typeof useAppStore.getState>['appState']) => ReturnType<typeof useAppStore.getState>['appState']) => void;
  startAnalysis: () => void;
}

/**
 * App-level event listeners extracted from App.tsx:
 * - Deep-link license activation (fourda://activate?key=...)
 * - Embedding status changes (degraded/unavailable toasts)
 * - Framework/Comparison page triggers (from AboutPanel)
 * - Mount-only cached result loader / auto-analysis trigger
 */
export function useAppListeners({
  addToast,
  setEmbeddingStatus,
  setShowFramework,
  setShowComparison,
  setState,
  startAnalysis,
}: AppListenersConfig) {
  const activateLicense = useAppStore(s => s.activateLicense);

  // Deep-link handler: fourda://activate?key=... (`4da` is retired: a URL
  // scheme must start with a letter, so browsers never launched it — and
  // `new URL('4da://...')` below would have thrown on it too.)
  //
  // Two arrival paths, same handler:
  //  - app already running: Windows starts a second instance with the URL in
  //    argv; the single-instance plugin (deep-link feature) forwards it into
  //    `deep-link://new-url`, app_setup validates and re-emits `deep-link-activate`.
  //  - app launched BY the link: the URL exists before any listener attaches,
  //    so app_setup parks it and take_pending_deep_link collects it on mount.
  useEffect(() => {
    const handleDeepLink = async (payload: string) => {
      try {
        const url = new URL(payload);
        if (url.hostname === 'activate' || url.pathname === '/activate') {
          const key = url.searchParams.get('key');
          if (key) {
            // fromDeepLink=true: the backend refuses to silently REPLACE a valid
            // licence for a different account (a fourda://activate link can be
            // fired by any site the user visits). First activation and same-account
            // renewal still activate normally.
            const proResult = await activateLicense(key, true);
            if (proResult.ok) {
              addToast('success', 'License activated successfully');
            } else if (proResult.reason === 'different_account') {
              addToast(
                'error',
                'This link is for a different 4DA account. To switch licences, open Settings → License and paste the key.',
              );
            } else {
              addToast('error', 'Invalid license key');
            }
          }
        }
      } catch {
        // Ignore malformed URLs
      }
    };

    const unlisten = safeListen<string>('deep-link-activate', (event) => {
      void handleDeepLink(event.payload);
    });

    void (async () => {
      if (useAppStore.getState().isBrowserMode) return; // no IPC in plain-browser mode
      try {
        const pending = await cmd('take_pending_deep_link');
        if (pending) await handleDeepLink(pending);
      } catch {
        // Command unavailable (e.g. stale backend) — cold-start links can
        // still be pasted manually; never block mount on this.
      }
    })();

    return () => { void unlisten.then(fn => fn()); };
  }, [activateLicense, addToast]);

  // Embedding status listener — surfaces degraded/unavailable state via toast
  useEffect(() => {
    const unlisten = safeListen<{ status: 'active' | 'degraded' | 'unavailable' }>('4da://embedding-status', (event) => {
      setEmbeddingStatus(event.payload.status);
      if (event.payload.status !== 'active') {
        addToast('warning', event.payload.status === 'degraded'
          ? 'Semantic scoring limited — embeddings using fallback'
          : 'Embedding service unavailable — using keyword signals only');
      }
    });
    return () => { void unlisten.then(fn => fn()); };
  }, [setEmbeddingStatus, addToast]);

  // Framework + Comparison page triggers (from AboutPanel via custom events)
  useEffect(() => {
    const frameworkHandler = () => setShowFramework(true);
    const comparisonHandler = () => setShowComparison(true);
    window.addEventListener('4da:show-framework', frameworkHandler);
    window.addEventListener('4da:show-comparison', comparisonHandler);
    return () => {
      window.removeEventListener('4da:show-framework', frameworkHandler);
      window.removeEventListener('4da:show-comparison', comparisonHandler);
    };
  }, [setShowFramework, setShowComparison]);

  // Global IPC timeout handler — surface timeout errors as toasts instead of silent failures
  // Uses .name check instead of instanceof to survive Vite code-splitting/minification
  useEffect(() => {
    const handler = (event: PromiseRejectionEvent) => {
      if (event.reason?.name === 'CommandTimeoutError') {
        event.preventDefault();
        addToast('error', 'Operation timed out. Please try again in a moment.');
      }
    };
    window.addEventListener('unhandledrejection', handler);
    return () => window.removeEventListener('unhandledrejection', handler);
  }, [addToast]);

  // On mount: load cached results from previous session, or auto-analyze
  useEffect(() => {
    let cancelled = false;
    const loadOrAnalyze = async () => {
      try {
        if (useAppStore.getState().isBrowserMode) return;
        if (await isVictauriDogfoodMode()) return;
        if (cancelled) return;

        const analysisState = await cmd('get_analysis_status');
        if (cancelled) return;

        if (analysisState.results && analysisState.results.length > 0) {
          const results = analysisState.results;
          const relevantCount = results.filter(r => r.relevant).length;
          setState(s => ({
            ...s,
            relevanceResults: results,
            nearMisses: analysisState.near_misses ?? null,
            status: `${relevantCount}/${results.length} items relevant (cached)`,
            analysisComplete: true,
            loading: false,
          }));
          return;
        }

        if (cancelled || useAppStore.getState().isFirstRun) return;
        const s = useAppStore.getState();
        // Cooldown: don't auto-analyze if we started recently (prevents hot-reload restart loops).
        // Only affects dev hot-reload — production cold starts clear sessionStorage automatically.
        const lastAutoAnalysis = Number(window.sessionStorage.getItem('4da-last-auto-analysis') ?? '0');
        if (Date.now() - lastAutoAnalysis < 15_000) return;
        if (!s.isFirstRun && !s.showOnboarding) {
          window.sessionStorage.setItem('4da-last-auto-analysis', String(Date.now()));
          startAnalysis();
        }
      } catch {
        // Silently ignore failures
      }
    };
    void loadOrAnalyze();
    return () => { cancelled = true; };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only
  }, []);
}
