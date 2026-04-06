// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../store';
import type { ToastType } from '../store/types';
import { cmd } from '../lib/commands';

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
 * - Deep-link license activation (4da://activate?key=...)
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
  const activateStreetsLicense = useAppStore(s => s.activateStreetsLicense);

  // Deep-link handler: 4da://activate?key=...
  useEffect(() => {
    const unlisten = listen<string>('deep-link-activate', async (event) => {
      try {
        const url = new URL(event.payload);
        if (url.hostname === 'activate' || url.pathname === '/activate') {
          const key = url.searchParams.get('key');
          if (key) {
            const proResult = await activateLicense(key);
            const streetsOk = await activateStreetsLicense(key);
            if (proResult.ok || streetsOk) {
              addToast('success', 'License activated successfully');
            } else {
              addToast('error', 'Invalid license key');
            }
          }
        }
      } catch {
        // Ignore malformed URLs
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, [activateLicense, activateStreetsLicense, addToast]);

  // Embedding status listener — surfaces degraded/unavailable state via toast
  useEffect(() => {
    const unlisten = listen<{ status: 'active' | 'degraded' | 'unavailable' }>('4da://embedding-status', (event) => {
      setEmbeddingStatus(event.payload.status);
      if (event.payload.status !== 'active') {
        addToast('warning', event.payload.status === 'degraded'
          ? 'Semantic scoring limited — embeddings using fallback'
          : 'Embedding service unavailable — using keyword signals only');
      }
    });
    return () => { unlisten.then(fn => fn()); };
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
        const command = event.reason?.command ?? 'unknown';
        addToast('error', `Operation timed out: ${command}. Please try again.`);
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
    loadOrAnalyze();
    return () => { cancelled = true; };
  // eslint-disable-next-line react-hooks/exhaustive-deps -- mount-only
  }, []);
}
