// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { DisagreementKind } from '../../types/analysis';

interface JudgesSplitBadgeProps {
  /** `null` / `undefined` hides the badge. */
  disagreement: DisagreementKind | null | undefined;
}

/**
 * Intelligence Mesh Phase 7 — "Judges Split" badge.
 *
 * Shown on items where the deterministic pipeline and the LLM advisor
 * disagreed by more than the reconciler threshold. The pipeline score
 * is always authoritative — this badge is a *transparency* signal, not
 * a filter. The design promise of the mesh is: you see the score you
 * get, and you know when the judges didn't agree on it.
 *
 * Renders nothing when `disagreement` is null/undefined. Matches the
 * inline pill style used by the rest of `BadgeRow.tsx` — small type,
 * subtle color, tooltip for detail. The tooltip varies per kind so
 * power users can read WHICH way the advisor leaned; the pill label
 * stays constant so the feed doesn't become visual noise.
 */
export const JudgesSplitBadge = memo(function JudgesSplitBadge({
  disagreement,
}: JudgesSplitBadgeProps) {
  const { t } = useTranslation();

  if (!disagreement) return null;

  const tooltip = (() => {
    switch (disagreement) {
      case 'AdvisorSkeptical':
        return t('results.judgesSplitSkeptical');
      case 'AdvisorEnthusiastic':
        return t('results.judgesSplitEnthusiastic');
      case 'AdvisorsInternal':
        return t('results.judgesSplitInternal');
    }
  })();

  return (
    <span
      className="flex-shrink-0 inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded font-medium bg-indigo-500/15 text-indigo-300 border border-indigo-500/20"
      title={tooltip}
      aria-label={tooltip}
      data-testid="judges-split-badge"
      data-disagreement-kind={disagreement}
    >
      <svg
        width="10"
        height="10"
        viewBox="0 0 16 16"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className="flex-shrink-0"
        aria-hidden="true"
      >
        {/* Two opposing arrows — pipeline up, advisor down (or vice versa) */}
        <path
          d="M4 2v8M4 10l-2-2M4 10l2-2"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path
          d="M12 14V6M12 6l-2 2M12 6l2 2"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
      {t('results.judgesSplit')}
    </span>
  );
});
