// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect, useState } from 'react';
import type { VoidSignal } from '../types';
import { cmd } from '../lib/commands';
import { safeListen } from '../lib/tauri-events';
import { WORKING_PULSE } from '../components/void-engine/signal-visuals';

/**
 * How long one piece of fetch evidence keeps the mark "working". During a
 * real fetch the heartbeat emits every ~1 s (throttled) as sources complete.
 * The staleness tick re-emits a DECAYED pulse every 60 s, so the window must
 * stay under that, and decayed values are never counted as evidence anyway.
 */
export const WORK_FRESHNESS_MS = 45_000;

/**
 * Is this void-signal update evidence that a fetch is in flight? Only a
 * working-level pulse that did not fall: fetch start (1.0) and per-source
 * progress (0.4 -> 1.0) rise or hold, while the once-a-minute staleness tick
 * only ever decays the last pulse (x0.98). Without this rule a missed final
 * pulse=0 would be re-freshened by every tick for ~50 minutes.
 */
export function isWorkEvidence(pulse: number, previousPulse: number): boolean {
  return pulse >= WORKING_PULSE && pulse >= previousPulse - 1e-6;
}

/**
 * True while the backend reports a background source fetch: the latest pulse
 * is at a working level AND fresh evidence of it arrived within
 * WORK_FRESHNESS_MS. A timer drops it back to false when the window passes
 * with no new evidence — it does not wait for a re-render.
 */
export function useBackgroundWork(): boolean {
  const [working, setWorking] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let lastPulse = 0;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const clear = () => {
      if (timer) clearTimeout(timer);
      timer = null;
    };
    const apply = (raw: number | undefined) => {
      if (cancelled) return;
      const pulse = typeof raw === 'number' && Number.isFinite(raw) ? raw : 0;
      const evidence = isWorkEvidence(pulse, lastPulse);
      lastPulse = pulse;
      if (pulse < WORKING_PULSE) {
        clear();
        setWorking(false);
      } else if (evidence) {
        clear();
        setWorking(true);
        timer = setTimeout(() => {
          timer = null;
          if (!cancelled) setWorking(false);
        }, WORK_FRESHNESS_MS);
      }
      // A decaying working-level pulse changes nothing: the timer from the
      // last real evidence decides.
    };

    let eventSeen = false;
    void cmd('get_void_signal')
      .then((s) => {
        // A live event already superseded this snapshot.
        if (!eventSeen) apply(s?.pulse);
      })
      .catch((e) => console.debug('[background-work] void signal not available:', e));
    const unlistenPromise = safeListen<VoidSignal>('void-signal', (event) => {
      eventSeen = true;
      apply(event.payload?.pulse);
    });

    return () => {
      cancelled = true;
      clear();
      void unlistenPromise.then((fn) => fn());
    };
  }, []);

  return working;
}
