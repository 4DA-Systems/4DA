// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { SignalUpgradeCTA } from '../SignalUpgradeCTA';

/**
 * Compact inline notice shown on the Preemption tab when the feed is the
 * free security floor (feed.tier_scope === 'free_floor'). The OSV-verified
 * tier renders normally above this; the locked tiers (AI-assessed, trend
 * chains) are summarized here with an upgrade path instead of a full-page
 * paywall. Honest by design: security baselines are never paywalled.
 */
export const PreemptionFreeFloorNotice = memo(function PreemptionFreeFloorNotice() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 px-4 py-3 rounded-lg bg-bg-secondary border border-accent-gold/20">
      <div className="min-w-0">
        <p className="text-sm font-medium text-white">{t('preemption.freeFloor.title')}</p>
        <p className="text-xs text-text-muted mt-0.5">{t('preemption.freeFloor.subtitle')}</p>
      </div>
      <div className="shrink-0">
        <SignalUpgradeCTA compact />
      </div>
    </div>
  );
});
