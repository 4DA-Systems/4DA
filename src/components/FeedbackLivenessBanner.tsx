// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd, type FeedbackLiveness } from '../lib/commands';
import { runWhenIdle } from '../lib/defer';

/**
 * Surfaced volume below which silence is expected, not a dead loop. Two weeks
 * of a working feed is several hundred surfaces; a fresh install or a quiet
 * corpus never reaches this, so the line cannot fire on day one.
 */
export const LIVENESS_MIN_SURFACED = 200;

/**
 * The one condition the banner speaks to: plenty surfaced, nothing came back
 * on EITHER feedback channel. A single rating or click in the window is enough
 * to stay silent — the point is a dead loop, not a quiet one.
 */
export function shouldShowFeedbackLiveness(l: FeedbackLiveness | null | undefined): boolean {
  if (!l) return false;
  return l.surfaced_14d >= LIVENESS_MIN_SURFACED && l.feedback_14d === 0 && l.interactions_14d === 0;
}

/**
 * One quiet line in the Signal tab when the human feedback loop is dead.
 *
 * The calibration monitor needs >= 10 ratings before it does anything and
 * says so only in the log ("cold start — staying silent", every 6h). Live
 * 2026-09-04: one `feedback` row ever, `interactions` stopped 2026-08-24,
 * `precision_stats.precision` NULL — while the tab surfaced ~760 items in two
 * weeks and nothing on screen said the scores were not calibrated to the user.
 * This is that sentence. Hide-when-quiet, like the card it sits above.
 */
export function FeedbackLivenessBanner() {
  const { t } = useTranslation();
  const [liveness, setLiveness] = useState<FeedbackLiveness | null>(null);

  useEffect(() => {
    let cancelled = false;
    // Off the first-paint IPC stampede — this is not paint-critical.
    const cancelIdle = runWhenIdle(() => {
      cmd('get_feedback_liveness')
        .then((l) => {
          if (!cancelled) setLiveness(l);
        })
        .catch(() => {
          // No data means no claim: the banner stays hidden.
        });
    });
    return () => {
      cancelled = true;
      cancelIdle();
    };
  }, []);

  if (!shouldShowFeedbackLiveness(liveness)) return null;

  return (
    <p
      role="status"
      data-testid="feedback-liveness-banner"
      className="mb-3 px-4 py-2 text-[11px] text-text-muted bg-bg-secondary border border-border rounded-lg"
    >
      {t(
        'missed.feedbackLiveness',
        'No ratings or clicks in 14 days. Scores are not calibrated to you yet.',
      )}
    </p>
  );
}
