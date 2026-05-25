// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { getScoreTier } from './types';

const ScoreBar = memo(function ScoreBar({ score }: { score: number }) {
  const { t } = useTranslation();

  if (score < 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-5">
        <div className="flex items-baseline gap-3 mb-3">
          <span className="text-lg font-medium text-text-muted">{t('blindspots.score.building')}</span>
        </div>
        <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
          <div className="h-full rounded-full bg-border w-1/4 animate-pulse" />
        </div>
      </div>
    );
  }

  const tier = getScoreTier(score);
  const pressure = Math.round(score);
  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-5">
      <div className="flex items-baseline gap-3 mb-3">
        <span className={`text-3xl font-semibold tabular-nums ${tier.color}`}>{pressure}</span>
        <span className="text-text-muted text-sm">/100</span>
        <span className={`text-sm ${tier.color}`}>{t(tier.labelKey)}</span>
      </div>
      <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-500 ${tier.bg}`}
          style={{ width: `${Math.max(0, 100 - pressure)}%` }}
        />
      </div>
    </div>
  );
});

export default ScoreBar;
