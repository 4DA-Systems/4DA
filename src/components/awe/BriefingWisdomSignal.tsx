// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';

// ============================================================================
// Types
// ============================================================================

interface Props {
  signals: Array<{ title: string; signal_type?: string | null }>;
}

// ============================================================================
// Sub-components
// ============================================================================

function PrecedentItem({ description, outcome }: { description: string; outcome: string }) {
  return (
    <li className="text-xs text-text-secondary leading-relaxed flex items-start gap-2">
      <span className="text-accent-gold mt-0.5 flex-shrink-0">{'\u25C6'}</span>
      <span>
        You decided &ldquo;{description}&rdquo; &mdash;{' '}
        <span className="text-text-muted italic">{outcome}</span>
      </span>
    </li>
  );
}

function PrincipleItem({ statement, confidence }: { statement: string; confidence: number }) {
  return (
    <li className="text-xs text-text-secondary leading-relaxed flex items-start gap-2">
      <span className="text-accent-gold/60 mt-0.5 flex-shrink-0">{'\u25C7'}</span>
      <span>
        {statement}
        <span className="text-text-muted ml-1">({Math.round(confidence * 100)}%)</span>
      </span>
    </li>
  );
}

function AntiPatternItem({ pattern, failureMode }: { pattern: string; failureMode: string }) {
  return (
    <li className="text-xs text-amber-400/80 leading-relaxed flex items-start gap-2">
      <span className="text-amber-400 mt-0.5 flex-shrink-0">{'\u26A0'}</span>
      <span>
        {pattern} &mdash;{' '}
        <span className="text-text-muted italic">{failureMode}</span>
      </span>
    </li>
  );
}

// ============================================================================
// Main Component
// ============================================================================

/**
 * BriefingWisdomSignal — pattern-match briefing signals against AWE decision history.
 *
 * Renders gold-bordered card with precedents, principles, and anti-pattern warnings
 * when AWE finds relevant matches for the current briefing signals.
 * Returns null when no matches are found.
 */
export const BriefingWisdomSignal = memo(function BriefingWisdomSignal({ signals }: Props) {
  const { t } = useTranslation();
  const awePatterns = useAppStore(s => s.awePatterns);
  const loadAwePatterns = useAppStore(s => s.loadAwePatterns);

  useEffect(() => {
    if (signals.length === 0) return;
    const query = signals
      .slice(0, 5)
      .map(s => s.title)
      .join(', ');
    void loadAwePatterns(query);
  }, [signals, loadAwePatterns]);

  // No matches — render nothing
  if (!awePatterns) return null;

  const hasPrecedents = awePatterns.precedents.length > 0;
  const hasPrinciples = awePatterns.principles.length > 0;
  const hasAntiPatterns = awePatterns.anti_patterns.length > 0;

  if (!hasPrecedents && !hasPrinciples && !hasAntiPatterns) return null;

  return (
    <div className="bg-bg-secondary border border-accent-gold/20 rounded-lg p-4 space-y-3">
      {/* Header */}
      <div className="flex items-center gap-2">
        <span className="text-accent-gold text-sm">{'\u25C7'}</span>
        <h4 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium">
          {t('awe.briefing.title')}
        </h4>
      </div>

      {/* Precedents */}
      {hasPrecedents && (
        <div>
          <h5 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-1.5">
            {t('awe.briefing.precedents')}
          </h5>
          <ul className="space-y-1.5">
            {awePatterns.precedents.map((p, i) => (
              <PrecedentItem key={i} description={p.description} outcome={p.outcome} />
            ))}
          </ul>
        </div>
      )}

      {/* Principles */}
      {hasPrinciples && (
        <div>
          <h5 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-1.5">
            {t('awe.briefing.principles')}
          </h5>
          <ul className="space-y-1.5">
            {awePatterns.principles.map((p, i) => (
              <PrincipleItem key={i} statement={p.statement} confidence={p.confidence} />
            ))}
          </ul>
        </div>
      )}

      {/* Anti-pattern warnings */}
      {hasAntiPatterns && (
        <div className="border-t border-amber-500/20 pt-3">
          <h5 className="text-[10px] text-amber-400 uppercase tracking-wider font-medium mb-1.5">
            {t('awe.briefing.antiPatterns')}
          </h5>
          <ul className="space-y-1.5">
            {awePatterns.anti_patterns.map((ap, i) => (
              <AntiPatternItem
                key={i}
                pattern={ap.pattern}
                failureMode={ap.failure_mode}
              />
            ))}
          </ul>
        </div>
      )}
    </div>
  );
});
