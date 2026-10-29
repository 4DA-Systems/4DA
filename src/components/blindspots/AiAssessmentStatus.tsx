// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { budgetResetVars, parseBudgetReset } from '../../utils/error-messages';
import { formatRelativeDate } from '../../utils/format-date';

/** The AI triage the Blind Spots view is showing, as far as its label needs. */
export interface ShownAssessment {
  model: string;
  /** Unix milliseconds the verdicts were produced (ts-rs types it bigint). */
  assessedAt: number | bigint | null;
  /** The persisted verdict predates the current security/breaking evidence. */
  stale: boolean;
}

/**
 * The line beside "Assess with AI": who assessed, how long ago, and whether
 * the verdicts on screen are still current.
 *
 * Audit L4 (2026-10-12): a 3.5-day-old cached verdict read "Assessed by
 * claude-sonnet-5" with no age and `stale` unsurfaced, and stood beside
 * "Assessment failed" as if a fresh run had just succeeded. The verdicts
 * stay on screen (they are the last triage the user has), but the label now
 * says they are an earlier one whenever the evidence moved since or the
 * re-run failed.
 */
export const AiAssessmentStatus = memo(function AiAssessmentStatus({
  shown,
  loading,
  error,
}: {
  shown: ShownAssessment | null;
  loading: boolean;
  error: string | null;
}) {
  const { t, i18n } = useTranslation();
  if (loading) return null;

  const failed = error !== null && error !== 'no_llm';
  const at = shown?.assessedAt != null ? Number(shown.assessedAt) : NaN;
  const age = Number.isFinite(at) && at > 0 ? formatRelativeDate(new Date(at)) : '';

  let assessedLabel: string | null = null;
  if (shown) {
    if (shown.stale) {
      assessedLabel = t('blindspots.ai.earlierChanged', { model: shown.model, age });
    } else if (failed) {
      assessedLabel = t('blindspots.ai.earlier', { model: shown.model, age });
    } else {
      assessedLabel = t('blindspots.ai.assessedAgo', { model: shown.model, age });
    }
  }

  let errorLabel: string | null = null;
  if (failed) {
    const reset = parseBudgetReset(error);
    errorLabel = reset
      ? t('blindspots.ai.budgetReached', budgetResetVars(reset, i18n.language ?? 'en'))
      : t('blindspots.ai.failed');
  }

  return (
    <>
      {assessedLabel && (
        <span
          className={`text-[10px] ${shown?.stale || failed ? 'text-amber-300' : 'text-text-muted'}`}
          data-testid="blindspots-ai-assessed"
        >
          {assessedLabel}
        </span>
      )}
      {error === 'no_llm' && (
        <span className="text-[10px] text-text-muted">{t('blindspots.ai.noModel')}</span>
      )}
      {errorLabel && <span className="text-[10px] text-red-400">{errorLabel}</span>}
    </>
  );
});
