// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useAppStore } from '../store';

/**
 * Event names — kept in sync with `src-tauri/src/awe_events.rs`.
 *
 * Every mutation to the AWE wisdom graph emits one of these events. The
 * frontend subscribes and updates its zustand store incrementally instead
 * of polling. Before this hook existed, the UI fetched `get_awe_summary`
 * once on component mount and then sat frozen — users reported the Wisdom
 * Trajectory "looked exactly the same all week" because it literally did.
 */
const EVENTS = {
  DECISION_ADDED: 'awe:decision-added',
  FEEDBACK_RECORDED: 'awe:feedback-recorded',
  PRINCIPLE_VALIDATED: 'awe:principle-validated',
  COVERAGE_CHANGED: 'awe:coverage-changed',
  SCAN_COMPLETE: 'awe:scan-complete',
  RETRIAGE_COMPLETE: 'awe:retriage-complete',
  SEED_COMPLETE: 'awe:seed-complete',
  SOURCE_MINING_COMPLETE: 'awe:source-mining-complete',
  SUMMARY_STALE: 'awe:summary-stale',
} as const;

// ----------------------------------------------------------------------------
// Payload shapes — mirror the Rust `AweEvent` enum variants exactly.
// ----------------------------------------------------------------------------

interface DecisionAddedPayload {
  kind: 'decision_added';
  id: string;
  statement: string;
  domain: string;
  source: string;
}

interface FeedbackRecordedPayload {
  kind: 'feedback_recorded';
  decision_id: string;
  outcome: string;
}

interface PrincipleValidatedPayload {
  kind: 'principle_validated';
  statement: string;
  confidence: number;
  evidence_count: number;
  domain: string;
}

interface CoverageChangedPayload {
  kind: 'coverage_changed';
  decisions: number;
  feedback_count: number;
  coverage_pct: number;
}

interface ScanCompletePayload {
  kind: 'scan_complete';
  repos_scanned: number;
  decisions_stored: number;
  outcomes_inferred: number;
}

interface RetriageCompletePayload {
  kind: 'retriage_complete';
  auto_confirmed: number;
  demoted: number;
  unchanged: number;
}

interface SeedCompletePayload {
  kind: 'seed_complete';
  decisions_loaded: number;
}

interface SourceMiningCompletePayload {
  kind: 'source_mining_complete';
  candidates_considered: number;
  decisions_created: number;
  rate_limited: number;
}

// ----------------------------------------------------------------------------
// Debounced refresh — when a burst of events arrives (e.g. daily autonomous
// tier job creates 12 decisions back-to-back), we coalesce into a single
// `loadAweSummary()` call instead of hammering the backend 12 times.
// ----------------------------------------------------------------------------

const REFRESH_DEBOUNCE_MS = 400;

function createDebouncer(fn: () => void, wait: number): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  return () => {
    if (timer != null) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      try {
        fn();
      } catch (err) {
        // Store actions can throw if the Tauri bridge is disconnected.
        // Losing a single refresh is non-fatal; the next event or the
        // 30 s poll safety net will pick up any missed state.
        console.warn('[awe-live-events] debounced refresh error', err);
      }
    }, wait);
  };
}

/**
 * Subscribe to all AWE live events. Call this once from a top-level
 * component that is always mounted during an AWE-aware session.
 * Events are broadcast; there is no hidden state.
 *
 * All state updates flow through the zustand store so any component
 * that reads `aweSummary`, `awePendingDecisions`, `aweWisdomWell`,
 * etc. will re-render automatically when data changes.
 */
export function useAweLiveEvents(): void {
  useEffect(() => {
    const unlistens: UnlistenFn[] = [];
    let cancelled = false;

    // Always re-fetch store actions via getState() inside each handler.
    // Caching a `store` reference at effect run time makes spies on
    // specific methods miss any later call that went through the cached
    // variable instead of the live state object. This pattern keeps the
    // hook testable without losing any performance — zustand's getState
    // is a trivial field read.
    const refreshSummary = createDebouncer(() => {
      void useAppStore.getState().loadAweSummary();
    }, REFRESH_DEBOUNCE_MS);

    const refreshAll = createDebouncer(() => {
      const s = useAppStore.getState();
      void s.loadAweSummary();
      void s.loadAwePendingDecisions(20);
      void s.loadAweWisdomWell();
    }, REFRESH_DEBOUNCE_MS);

    const setup = async () => {
      const handlers: Array<[string, (payload: unknown) => void]> = [
        [EVENTS.DECISION_ADDED, (raw) => {
          const payload = raw as DecisionAddedPayload;
          void payload;
          refreshAll();
        }],
        [EVENTS.FEEDBACK_RECORDED, (raw) => {
          const payload = raw as FeedbackRecordedPayload;
          void payload;
          refreshAll();
        }],
        [EVENTS.PRINCIPLE_VALIDATED, (raw) => {
          const payload = raw as PrincipleValidatedPayload;
          // Principle validated is the flagship user-visible event.
          // Refresh the wisdom well so the new principle shows up in
          // the "Forming intelligence" list immediately.
          useAppStore.getState().addToast(
            'success',
            `New principle: ${payload.statement.slice(0, 80)}`,
          );
          refreshAll();
        }],
        [EVENTS.COVERAGE_CHANGED, (raw) => {
          const payload = raw as CoverageChangedPayload;
          void payload;
          refreshSummary();
        }],
        [EVENTS.SCAN_COMPLETE, (raw) => {
          const payload = raw as ScanCompletePayload;
          if (payload.decisions_stored > 0) {
            refreshAll();
          } else {
            refreshSummary();
          }
        }],
        [EVENTS.RETRIAGE_COMPLETE, (raw) => {
          const payload = raw as RetriageCompletePayload;
          if (payload.auto_confirmed > 0 || payload.demoted > 0) {
            refreshAll();
          }
        }],
        [EVENTS.SEED_COMPLETE, (raw) => {
          const payload = raw as SeedCompletePayload;
          if (payload.decisions_loaded > 0) {
            useAppStore.getState().addToast(
              'info',
              `Wisdom engine seeded with ${payload.decisions_loaded} decisions`,
            );
            refreshAll();
          }
        }],
        [EVENTS.SOURCE_MINING_COMPLETE, (raw) => {
          const payload = raw as SourceMiningCompletePayload;
          if (payload.decisions_created > 0) {
            refreshAll();
          }
        }],
        [EVENTS.SUMMARY_STALE, () => {
          refreshSummary();
        }],
      ];

      for (const [event, handler] of handlers) {
        try {
          const unlisten = await listen(event, (e) => {
            try {
              handler(e.payload);
            } catch (err) {
              // Never let a handler error break the subscription chain
              console.warn('[awe-live-events] handler error', event, err);
            }
          });
          if (cancelled) {
            unlisten();
          } else {
            unlistens.push(unlisten);
          }
        } catch (err) {
          console.warn('[awe-live-events] failed to subscribe to', event, err);
        }
      }
    };

    void setup();

    return () => {
      cancelled = true;
      for (const unlisten of unlistens) {
        try {
          unlisten();
        } catch {
          // Best effort
        }
      }
    };
  }, []);
}
