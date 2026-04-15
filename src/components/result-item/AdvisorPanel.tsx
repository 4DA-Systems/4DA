import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { AdvisorSignal, DisagreementKind } from '../../types/analysis';

interface AdvisorPanelProps {
  /** Zero or more advisor opinions stamped with provenance. */
  advisorSignals: AdvisorSignal[] | undefined;
  /** Disagreement flag from the reconciler (Phase 2). */
  disagreement: DisagreementKind | null | undefined;
}

/**
 * Intelligence Mesh Phase 7b — "Advisors" panel inside the score drawer.
 *
 * Surfaces every LLM advisor's opinion on this item with full provenance:
 * model identity, prompt_version, raw + normalized scores, and reasoning.
 * The deterministic pipeline score is authoritative; this panel shows
 * what the advisors thought and whether they agreed — the full "receipts"
 * promise from INTELLIGENCE-MESH.md §3.1.
 *
 * Rendering rules:
 *   - Null/undefined/empty `advisorSignals` AND no `disagreement` → hide.
 *     Items that never passed rerank have no receipts to show.
 *   - Disagreement banner (if set) appears above the advisor list with
 *     a kind-specific explanation. This repeats the badge's promise
 *     ("pipeline score stands") at the detail layer.
 *   - Each advisor row: provider/model, normalized score as a percentage,
 *     confidence, and — when present — the one-sentence reason.
 *   - Uncalibrated advisors carry a "pre-mesh" tag (Phase 5 calibration
 *     will replace this sentinel with a real curve ID).
 */
const PRE_MESH_CALIBRATION = 'pre-mesh-unknown';

export const AdvisorPanel = memo(function AdvisorPanel({
  advisorSignals,
  disagreement,
}: AdvisorPanelProps) {
  const { t } = useTranslation();
  const signals = advisorSignals ?? [];
  if (signals.length === 0 && !disagreement) return null;

  const disagreementText = (() => {
    switch (disagreement) {
      case 'AdvisorSkeptical':
        return t('scoreDrawer.advisorSkeptical');
      case 'AdvisorEnthusiastic':
        return t('scoreDrawer.advisorEnthusiastic');
      case 'AdvisorsInternal':
        return t('scoreDrawer.advisorsInternal');
      default:
        return null;
    }
  })();

  return (
    <section
      className="space-y-2"
      data-testid="advisor-panel"
      aria-labelledby={`advisor-panel-heading-${signals.length}`}
    >
      <div className="flex items-center gap-2">
        <span
          id={`advisor-panel-heading-${signals.length}`}
          className="text-[10px] text-text-muted uppercase tracking-wider"
        >
          {t('scoreDrawer.advisors')}
        </span>
        {disagreement && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-indigo-500/15 text-indigo-300 border border-indigo-500/20 font-medium">
            {t('scoreDrawer.split')}
          </span>
        )}
      </div>

      {disagreementText && (
        <p
          className="text-xs text-text-secondary italic"
          data-testid="advisor-panel-disagreement"
        >
          {disagreementText}
        </p>
      )}

      {signals.length > 0 && (
        <ul className="space-y-1.5">
          {signals.map((sig, idx) => {
            const normalizedPct = Math.round(sig.normalized_score * 100);
            const confidencePct = Math.round(sig.confidence * 100);
            const uncalibrated =
              !sig.calibration_id || sig.calibration_id === PRE_MESH_CALIBRATION;
            return (
              <li
                key={`${sig.provider}-${sig.model}-${idx}`}
                className="rounded border border-border/50 bg-bg-tertiary/50 px-2.5 py-1.5"
                data-testid="advisor-row"
                data-provider={sig.provider}
                data-model={sig.model}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="min-w-0 flex items-center gap-1.5">
                    <span className="text-xs text-text-secondary font-mono truncate">
                      {sig.provider}/{sig.model}
                    </span>
                    {uncalibrated && (
                      <span
                        className="text-[9px] px-1 py-0.5 rounded bg-amber-500/10 text-amber-400/80 uppercase"
                        title={t('scoreDrawer.preMeshTooltip')}
                      >
                        {t('scoreDrawer.preMesh')}
                      </span>
                    )}
                  </div>
                  <span className="text-xs font-mono text-white flex-shrink-0">
                    {normalizedPct}%
                  </span>
                </div>
                <div className="flex items-center gap-2 mt-0.5 text-[10px] text-text-muted">
                  <span>
                    {t('scoreDrawer.advisorConfidence', { pct: confidencePct })}
                  </span>
                  {sig.prompt_version && (
                    <span
                      className="font-mono truncate"
                      title={t('scoreDrawer.promptVersionTooltip')}
                    >
                      {sig.prompt_version}
                    </span>
                  )}
                </div>
                {sig.reason && (
                  <p
                    className="text-[11px] text-text-secondary mt-1"
                    data-testid="advisor-reason"
                  >
                    {sig.reason}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
});
