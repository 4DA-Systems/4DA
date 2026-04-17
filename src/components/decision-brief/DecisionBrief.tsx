// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Decision Brief — Intelligence Reconciliation Phase 10 (2026-04-17).
 *
 * The canonical AWE output artifact. One scrollable card with five
 * fixed sections:
 *   1. The decision, in one line (AWE's restatement)
 *   2. Three things to check you haven't considered
 *   3. What changes if you're wrong (reversibility + worst case)
 *   4. Similar decisions you or the industry have made (precedents)
 *   5. The judgment (one line, confidence + provenance tag)
 *
 * This is the ONLY user-visible AWE surface permitted by the
 * Intelligence Doctrine (rule 5). Everything else about AWE is
 * infrastructure.
 */

import { memo } from 'react';
import { useTranslation } from 'react-i18next';

// ============================================================================
// Types
// ============================================================================

export interface BriefPrecedent {
  statement: string;
  outcome: 'confirmed' | 'refuted' | 'partial' | 'pending';
  origin: string;
  similarity: number;
}

export interface DecisionBriefData {
  /** AWE's one-line restatement of the decision. */
  decision: string;
  /** Three assumptions or biases the user should verify. */
  assumptions: string[];
  /** Reversibility score 0-1 (0 = fully reversible). */
  reversibility?: number;
  /** Worst-case / second-order path in a sentence. */
  worstCase?: string;
  /** Up to 3 matched precedents. */
  precedents: BriefPrecedent[];
  /** One-line judgment. */
  verdict: string;
  /** 0-1 confidence. */
  confidence: number;
  /** "checklist" | "heuristic" | "calibrated" | "llm_assessed" */
  confidenceProvenance: string;
  /** "structured" | "voice" | "challenge" */
  mode: string;
}

// ============================================================================
// Small helpers
// ============================================================================

const OUTCOME_COLOR: Record<BriefPrecedent['outcome'], string> = {
  confirmed: 'text-green-400',
  refuted: 'text-red-400',
  partial: 'text-amber-400',
  pending: 'text-text-muted',
};

function provenanceLabel(provenance: string): string {
  switch (provenance) {
    case 'calibrated':
      return 'calibrated';
    case 'llm_assessed':
      return 'LLM-assessed';
    case 'checklist':
      return 'checklist';
    case 'heuristic':
    default:
      return 'heuristic';
  }
}

function reversibilityLabel(r?: number): { text: string; color: string } {
  if (r === undefined) return { text: '—', color: 'text-text-muted' };
  if (r >= 0.8) return { text: 'irreversible', color: 'text-red-400' };
  if (r >= 0.6) return { text: 'hard to reverse', color: 'text-orange-400' };
  if (r >= 0.3) return { text: 'reversible with effort', color: 'text-amber-400' };
  return { text: 'easily reversible', color: 'text-green-400' };
}

// ============================================================================
// Component
// ============================================================================

interface Props {
  data: DecisionBriefData;
  /** Optional action handlers for the Accept / Defer / Reject buttons. */
  onAccept?: () => void;
  onDefer?: () => void;
  onReject?: () => void;
}

export const DecisionBrief = memo(function DecisionBrief({
  data,
  onAccept,
  onDefer,
  onReject,
}: Props) {
  const { t } = useTranslation();
  const rev = reversibilityLabel(data.reversibility);
  const confidencePct = Math.round(Math.max(0, Math.min(1, data.confidence)) * 100);

  return (
    <article
      className="bg-bg-secondary rounded-lg border border-border p-6 max-w-2xl space-y-5"
      role="region"
      aria-label={t('decisionBrief.title', 'Decision Brief')}
    >
      {/* Section 1 — The decision */}
      <section>
        <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
          {t('decisionBrief.decision', 'The decision')}
        </h2>
        <p className="text-base text-white font-medium leading-snug">
          {data.decision}
        </p>
      </section>

      {/* Section 2 — Check these */}
      {data.assumptions.length > 0 && (
        <section>
          <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-2">
            {t('decisionBrief.assumptions', 'Check these')}
          </h2>
          <ul className="space-y-1.5">
            {data.assumptions.slice(0, 3).map((a, i) => (
              <li key={i} className="flex items-start gap-2 text-sm text-text-secondary">
                <span className="text-accent-gold mt-0.5 text-xs" aria-hidden="true">
                  ◆
                </span>
                <span className="leading-relaxed">{a}</span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* Section 3 — What changes if you're wrong */}
      <section>
        <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-2">
          {t('decisionBrief.whatIfWrong', "What changes if you're wrong")}
        </h2>
        <div className="flex items-baseline gap-3 text-sm">
          <span className={`font-medium ${rev.color}`}>{rev.text}</span>
          {data.reversibility !== undefined && (
            <span className="text-xs text-text-muted tabular-nums">
              {Math.round(data.reversibility * 100)}%
            </span>
          )}
        </div>
        {data.worstCase && (
          <p className="mt-1.5 text-xs text-text-secondary leading-relaxed">
            {data.worstCase}
          </p>
        )}
      </section>

      {/* Section 4 — Precedents */}
      {data.precedents.length > 0 && (
        <section>
          <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-2">
            {t('decisionBrief.precedents', 'Similar decisions')}
          </h2>
          <ul className="space-y-2">
            {data.precedents.slice(0, 3).map((p, i) => (
              <li key={i} className="text-sm">
                <div className="flex items-baseline gap-2">
                  <span className="text-white">{p.statement}</span>
                  <span className={`text-[10px] uppercase ${OUTCOME_COLOR[p.outcome]}`}>
                    {p.outcome}
                  </span>
                </div>
                <div className="text-[10px] text-text-muted mt-0.5">
                  {p.origin} · similarity {Math.round(p.similarity * 100)}%
                </div>
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* Section 5 — Verdict */}
      <section className="border-t border-border pt-4">
        <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-1.5">
          {t('decisionBrief.verdict', 'Verdict')}
        </h2>
        <p className="text-sm text-white leading-snug">{data.verdict}</p>
        <div className="mt-1.5 flex items-center gap-2 text-[11px] text-text-muted">
          <span className="tabular-nums">
            {t('decisionBrief.confidence', 'confidence')} {confidencePct}%
          </span>
          <span aria-hidden="true">·</span>
          <span className="italic">[{provenanceLabel(data.confidenceProvenance)}]</span>
        </div>
      </section>

      {/* Actions */}
      {(onAccept || onDefer || onReject) && (
        <section className="flex flex-wrap gap-2 pt-2">
          {onAccept && (
            <button
              type="button"
              onClick={onAccept}
              className="px-3 py-1.5 text-xs rounded-md bg-accent-gold/20 border border-accent-gold/40 text-accent-gold hover:bg-accent-gold/30 transition-colors"
            >
              {t('decisionBrief.accept', 'Accept')}
            </button>
          )}
          {onDefer && (
            <button
              type="button"
              onClick={onDefer}
              className="px-3 py-1.5 text-xs rounded-md border border-border bg-bg-tertiary text-text-secondary hover:text-white transition-colors"
            >
              {t('decisionBrief.defer', 'Defer')}
            </button>
          )}
          {onReject && (
            <button
              type="button"
              onClick={onReject}
              className="px-3 py-1.5 text-xs rounded-md border border-border bg-bg-tertiary text-text-secondary hover:text-white transition-colors"
            >
              {t('decisionBrief.reject', 'Reject')}
            </button>
          )}
        </section>
      )}
    </article>
  );
});

export default DecisionBrief;
