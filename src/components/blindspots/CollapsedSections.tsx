// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Collapsed "group" sections for the Blind Spots view, split out of
// StackCoverageMap.tsx to keep that file under the 500-line limit. All three
// render a collapsed header that expands to DepCoverageRow lists.
import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { DepRow } from './types';
import { DepCoverageRow } from './StackCoverageMap';

export const CoveredSection = memo(function CoveredSection({
  depRows, onDismissSignal,
}: {
  depRows: DepRow[];
  onDismissSignal: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [showCovered, setShowCovered] = useState(false);
  const [detailView, setDetailView] = useState(false);

  if (depRows.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setShowCovered(prev => !prev)}
        className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
      >
        <div className="w-2 h-2 rounded-full bg-green-400" />
        <h3 className="text-sm font-medium text-text-primary flex-1 text-left">
          {t('blindspots.covered.title')} ({depRows.length})
        </h3>
        <span className="text-[10px] text-green-400">
          {showCovered ? t('blindspots.covered.hide') : t('blindspots.covered.show')}
        </span>
      </button>
      {showCovered && (
        <div className="border-t border-border">
          {!detailView ? (
            <div className="px-4 py-3">
              <div className="flex flex-wrap gap-1.5">
                {depRows.map(dep => (
                  <span
                    key={dep.name}
                    className="text-[11px] px-2 py-1 rounded bg-green-500/8 text-green-400/80 border border-green-500/10"
                  >
                    {dep.name}
                  </span>
                ))}
              </div>
              {depRows.some(dep => dep.signals.length > 0) && (
                <button
                  onClick={(e) => { e.stopPropagation(); setDetailView(true); }}
                  className="mt-2.5 text-[10px] text-text-muted hover:text-green-400 transition-colors"
                >
                  {t('blindspots.covered.details')}
                </button>
              )}
            </div>
          ) : (
            <div>
              <button
                onClick={(e) => { e.stopPropagation(); setDetailView(false); }}
                className="w-full px-4 py-1.5 text-[10px] text-text-muted hover:text-green-400 transition-colors text-right"
              >
                {t('blindspots.covered.compact')}
              </button>
              {depRows.map(dep => (
                <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
});

/**
 * Phase 2c: a collapsed group for dependencies whose coverage gap applies only
 * to a build target the user does NOT build on the host (e.g. a `cfg(not(windows))`
 * crate on Windows). Surfaced, de-prioritised, never hidden — a cross-platform
 * dev can expand it. Modeled on `CoveredSection`.
 */
export const OtherBuildTargetsSection = memo(function OtherBuildTargetsSection({
  depRows, onDismissSignal, onAddWatch,
}: {
  depRows: DepRow[];
  onDismissSignal: (id: string) => void;
  onAddWatch?: (packageName: string, ecosystem: string) => void;
}) {
  const { t } = useTranslation();
  const [show, setShow] = useState(false);

  if (depRows.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setShow(prev => !prev)}
        aria-expanded={show}
        className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
      >
        <div className="w-2 h-2 rounded-full bg-[#8A8A8A]" />
        <h3 className="text-sm font-medium text-text-secondary flex-1 text-left">
          {t('blindspots.otherTargets.show', { count: depRows.length })}
        </h3>
        <span className="text-[10px] text-text-muted">
          {show ? t('blindspots.otherTargets.hide') : t('blindspots.otherTargets.expand')}
        </span>
      </button>
      {show && (
        <div className="border-t border-border">
          {depRows.map(dep => (
            <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} onAddWatch={onAddWatch} />
          ))}
        </div>
      )}
    </div>
  );
});

/**
 * Phase B: the "probably fine" bucket after an AI triage — dependencies the
 * model judged not worth the developer's attention right now. Collapsed to a
 * chip list (these are noise by definition); expandable for the full rows +
 * their one-line AI reasons. Modeled on `CoveredSection`.
 */
export const ProbablyFineSection = memo(function ProbablyFineSection({
  depRows, onDismissSignal, onAddWatch, aiRecommendations,
}: {
  depRows: DepRow[];
  onDismissSignal: (id: string) => void;
  onAddWatch?: (packageName: string, ecosystem: string) => void;
  aiRecommendations?: Map<string, string>;
}) {
  const { t } = useTranslation();
  const [show, setShow] = useState(false);
  const [detail, setDetail] = useState(false);

  if (depRows.length === 0) return null;

  return (
    <div className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <button
        onClick={() => setShow(prev => !prev)}
        aria-expanded={show}
        className="w-full px-4 py-3 flex items-center gap-2 hover:bg-bg-tertiary/30 transition-colors"
      >
        <div className="w-2 h-2 rounded-full bg-emerald-400/70" />
        <h3 className="text-sm font-medium text-text-secondary flex-1 text-left">
          {t('blindspots.ai.probablyFine', { count: depRows.length })}
        </h3>
        <span className="text-[10px] text-text-muted">
          {show ? t('blindspots.covered.hide') : t('blindspots.covered.show')}
        </span>
      </button>
      {show && (
        <div className="border-t border-border">
          {!detail ? (
            <div className="px-4 py-3">
              <div className="flex flex-wrap gap-1.5">
                {depRows.map(dep => (
                  <span
                    key={dep.name}
                    className="text-[11px] px-2 py-1 rounded bg-bg-tertiary/60 text-text-muted border border-border"
                    title={aiRecommendations?.get(dep.name) ?? undefined}
                  >
                    {dep.name}
                  </span>
                ))}
              </div>
              <button
                onClick={(e) => { e.stopPropagation(); setDetail(true); }}
                className="mt-2.5 text-[10px] text-text-muted hover:text-text-secondary transition-colors"
              >
                {t('blindspots.covered.details')}
              </button>
            </div>
          ) : (
            <div>
              <button
                onClick={(e) => { e.stopPropagation(); setDetail(false); }}
                className="w-full px-4 py-1.5 text-[10px] text-text-muted hover:text-text-secondary transition-colors text-right"
              >
                {t('blindspots.covered.compact')}
              </button>
              {depRows.map(dep => (
                <DepCoverageRow key={dep.name} dep={dep} onDismissSignal={onDismissSignal} onAddWatch={onAddWatch} aiRecommendation={aiRecommendations?.get(dep.name)} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
});
