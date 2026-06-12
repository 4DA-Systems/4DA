// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import type { BlindSpotTeaser } from '../../../src-tauri/bindings/bindings/BlindSpotTeaser';

import { SignalUpgradeCTA } from '../SignalUpgradeCTA';

/**
 * Paywalled state for the Blind Spots lens, with an honest free teaser.
 *
 * The full report stays Signal-gated, but `get_blind_spot_teaser` is free
 * and returns real aggregate counts from the same cached report path. When
 * the counts are nonzero we show them ("7 uncovered dependencies — Signal
 * shows which") above the upgrade CTA — real, actionable numbers, not
 * vanity metrics (doctrine rule 3). When the report is cold-start-suppressed
 * or all counts are zero, the plain paywall renders unchanged — never a
 * "no data yet" state (doctrine rule 6).
 */
export const BlindSpotsPaywall = memo(function BlindSpotsPaywall() {
  const { t } = useTranslation();
  const [teaser, setTeaser] = useState<BlindSpotTeaser | null>(null);

  useEffect(() => {
    let cancelled = false;
    cmd('get_blind_spot_teaser')
      .then((res) => {
        if (!cancelled) setTeaser(res);
      })
      .catch(() => {
        // Teaser is purely additive — on any failure the plain paywall stands.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const showTeaser =
    teaser !== null &&
    !teaser.cold_start &&
    (teaser.uncovered_count > 0 || teaser.stale_topic_count > 0 || teaser.missed_signal_count > 0);

  return (
    <div className="space-y-4" role="tabpanel" id="view-panel-blindspots" aria-labelledby="tab-blindspots">
      <header className="mb-2">
        <h1 className="text-xl font-semibold text-text-primary tracking-tight">{t('blindspots.title')}</h1>
        <p className="text-sm text-text-muted mt-1">{t('blindspots.subtitle')}</p>
      </header>
      <div className="flex flex-col items-center justify-center py-20 text-center gap-3">
        <div className="w-12 h-12 rounded-full bg-accent-gold/10 border border-accent-gold/20 flex items-center justify-center mb-1">
          <span className="text-accent-gold text-lg" aria-hidden="true">&#x1F512;</span>
        </div>
        <p className="text-sm font-medium text-text-primary">{t('blindspots.locked.title')}</p>
        <p className="text-xs text-text-muted max-w-sm">{t('blindspots.locked.subtitle')}</p>
        {showTeaser && (
          <div className="flex flex-col items-center gap-1.5 mt-2" data-testid="blindspots-teaser">
            {teaser.uncovered_count > 0 && (
              <p className="text-sm text-text-secondary tabular-nums">
                {t('blindspots.teaser.uncovered', { count: teaser.uncovered_count })}
              </p>
            )}
            {teaser.stale_topic_count > 0 && (
              <p className="text-sm text-text-secondary tabular-nums">
                {t('blindspots.teaser.staleTopics', { count: teaser.stale_topic_count })}
              </p>
            )}
            {teaser.missed_signal_count > 0 && (
              <p className="text-sm text-text-secondary tabular-nums">
                {t('blindspots.teaser.missedSignals', { count: teaser.missed_signal_count })}
              </p>
            )}
          </div>
        )}
        <div className="mt-1">
          <SignalUpgradeCTA />
        </div>
      </div>
    </div>
  );
});
