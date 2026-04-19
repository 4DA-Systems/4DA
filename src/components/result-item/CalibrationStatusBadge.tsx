// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../../lib/commands';
import type { CurveStatus } from '../../types/calibration';

interface CalibrationStatusBadgeProps {
  /** Full SHA-256 identity hash of the advisor model. When absent, the
   *  badge falls back to the pre-mesh tag because we can't look up a
   *  curve without the hash. */
  identityHash: string | null | undefined;
  /** Task the advisor performed — "judge" today. */
  task: string | null | undefined;
  /** Current prompt version reported by the advisor. Lets the backend
   *  mark the loaded curve stale when it was fit against an older
   *  prompt. */
  promptVersion: string | null | undefined;
  /** `calibration_id` on the AdvisorSignal. When it's the pre-mesh
   *  sentinel or null, the badge renders "uncalibrated" without doing
   *  a lookup — the advisor never produced a curve for this pair. */
  calibrationId: string | null | undefined;
}

const PRE_MESH_CALIBRATION = 'pre-mesh-unknown';

/**
 * Intelligence Mesh Phase 7c — calibration status badge.
 *
 * Shown inline next to the advisor's provider/model label. Three states:
 *
 *   - No curve on disk (pre-mesh, or fitter hasn't produced one yet):
 *     amber "uncalibrated" tag + explanatory tooltip.
 *   - Fresh curve: neutral "calibrated" tag + tooltip showing sample
 *     count, Brier, ECE, and fit age.
 *   - Stale curve (prompt_version drift): indigo "recalibrating" tag
 *     so users see that the mesh is aware and will self-heal.
 *
 * The badge listens for `calibration-curves-updated` and refetches so
 * the UI stays live after a manual or scheduled refit. Costs: one
 * cheap DB-less IPC call per render (the backend reads the curve file
 * from disk; file reads are <1ms typical).
 */
export const CalibrationStatusBadge = memo(function CalibrationStatusBadge({
  identityHash,
  task,
  promptVersion,
  calibrationId,
}: CalibrationStatusBadgeProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<CurveStatus | null>(null);
  const [loaded, setLoaded] = useState(false);

  // A calibration_id of pre-mesh-unknown or null means the advisor
  // itself reported no curve applied — we render uncalibrated without
  // making the backend call. This matters for correctness AND cost:
  // an advisor batch of 20 items would otherwise trigger 20 lookups
  // per item, and every one would return None.
  // Explicit null/empty handling (not truthy-coercion) so lint strict-
  // boolean-expressions is happy AND the intent is unambiguous.
  const canFetch =
    identityHash != null &&
    identityHash !== '' &&
    task != null &&
    task !== '' &&
    promptVersion != null &&
    promptVersion !== '' &&
    calibrationId != null &&
    calibrationId !== '' &&
    calibrationId !== PRE_MESH_CALIBRATION;

  useEffect(() => {
    if (
      !canFetch ||
      identityHash == null ||
      task == null ||
      promptVersion == null
    ) {
      setStatus(null);
      setLoaded(true);
      return;
    }

    let cancelled = false;

    const fetchStatus = async (): Promise<void> => {
      try {
        const s = await cmd('get_calibration_curve_status', {
          identityHash,
          task,
          currentPromptVersion: promptVersion,
        });
        if (!cancelled) {
          setStatus(s);
          setLoaded(true);
        }
      } catch {
        // Silent fallback: on any error, degrade to uncalibrated
        // display rather than block the whole drawer from rendering.
        if (!cancelled) {
          setStatus(null);
          setLoaded(true);
        }
      }
    };

    void fetchStatus();

    // Live refresh: when a scheduled or manual refit lands, refetch
    // so the "fit 2d ago" text doesn't go stale and the "recalibrating"
    // tag clears.
    const unlistenPromise = listen('calibration-curves-updated', () => {
      if (!cancelled) void fetchStatus();
    });

    return () => {
      cancelled = true;
      void unlistenPromise.then(u => {
        u();
      }).catch(() => {});
    };
  }, [canFetch, identityHash, task, promptVersion]);

  // Render path 1: advisor is pre-mesh or the lookup returned null.
  // Show the existing "uncalibrated" amber tag so we don't regress
  // the information the user already sees.
  if (!canFetch || (loaded && !status)) {
    return (
      <span
        className="text-[9px] px-1 py-0.5 rounded bg-amber-500/10 text-amber-400/80 uppercase"
        title={t('scoreDrawer.preMeshTooltip')}
        data-testid="calibration-badge"
        data-calibration-state="pre-mesh"
      >
        {t('scoreDrawer.preMesh')}
      </span>
    );
  }

  // Render path 2: fetch not yet complete — render nothing rather than
  // a skeleton. Badges flicker quickly on fast lookups and a skeleton
  // pulse would be more distracting than helpful.
  if (!loaded || !status) return null;

  // Render path 3: stale curve (prompt drifted). The apply layer has
  // already invalidated it; we tell the user the mesh is aware.
  if (status.is_stale) {
    return (
      <span
        className="text-[9px] px-1 py-0.5 rounded bg-indigo-500/15 text-indigo-300 border border-indigo-500/20 uppercase"
        title={t('scoreDrawer.staleTooltip')}
        data-testid="calibration-badge"
        data-calibration-state="stale"
      >
        {t('scoreDrawer.stale')}
      </span>
    );
  }

  // Render path 4: fresh, usable curve. Show compact metrics on hover.
  const brier = status.brier_score.toFixed(2);
  const ece = status.ece.toFixed(2);
  const tooltip = t('scoreDrawer.calibratedTooltip', {
    samples: status.sample_count,
    brier,
    ece,
    days: status.age_days,
  });

  return (
    <span
      className="text-[9px] px-1 py-0.5 rounded bg-emerald-500/10 text-emerald-400/80 uppercase"
      title={tooltip}
      data-testid="calibration-badge"
      data-calibration-state="calibrated"
      data-curve-id={status.curve_id}
    >
      {t('scoreDrawer.calibrated')}
    </span>
  );
});
