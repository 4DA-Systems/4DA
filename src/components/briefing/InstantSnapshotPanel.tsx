// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo } from 'react';
import { isSafeUrl } from '../../utils/sanitize-html';
import { useTranslation } from 'react-i18next';
import { getRelevancePresentation } from '../../utils/score';
import { isAbstentionSynthesis } from './briefing-synthesis-helpers';
import type { InstantBriefingSnapshot } from '../../store/types';

interface InstantSnapshotPanelProps {
  snapshot: InstantBriefingSnapshot;
}

/**
 * Sovereign Cold Boot — renders yesterday's pre-baked briefing snapshot
 * for instant first paint while fresh intelligence loads in the background.
 */
export const InstantSnapshotPanel = memo(function InstantSnapshotPanel({
  snapshot,
}: InstantSnapshotPanelProps) {
  const { t } = useTranslation();

  // A cached *abstention* ("Low signal — no noteworthy intelligence overnight")
  // carries nothing worth pre-painting. Echoing yesterday's verdict dead-center
  // while a fresh analysis is already in flight reads as a definitive "nothing's
  // happening" when the truth is "today's scan hasn't landed yet" — and asserts a
  // verdict the system hasn't re-derived (violates Accurate-first). So when the
  // snapshot is an abstention we render an honest *working* state instead; the real
  // low-signal verdict, if today warrants it, surfaces once live analysis completes.
  const isAbstention = isAbstentionSynthesis(snapshot.synthesis);

  return (
    <section aria-label={t('briefing.dailyOverview')} className="bg-bg-primary rounded-lg space-y-4">
      <div className="bg-bg-secondary rounded-lg border border-border">
        <div className="px-5 pt-5 pb-3 border-b border-border flex items-center justify-between gap-3">
          <h2 className="text-[9px] font-semibold tracking-[0.12em] text-text-muted uppercase">
            {t('briefing.intelligenceBriefing')}
          </h2>
          <div
            className="flex items-center gap-2 text-[10px] text-text-muted"
            title={t('briefing.refreshingInBackground', 'Refreshing intelligence in background')}
          >
            <span className="inline-block w-1.5 h-1.5 rounded-full bg-[#D4AF37] animate-pulse" />
            <span>{snapshot.generatedAtDisplay}</span>
          </div>
        </div>
        <div className="p-5 space-y-4">
          {/*
            Synthesis has two render shapes:
            1. Abstention — "Low signal — no noteworthy intelligence overnight"
               Render as a single muted message with NO source-items list.
               The brief is deliberately saying "nothing worth saying today";
               echoing a junk-items list below would undermine that verdict.
            2. Normal three-section briefing — render as prose, followed by
               the "Source items" list with an explicit label so the user
               knows these are the underlying data, not independent bullets.
          */}
          {isAbstention ? (
            <div className="py-8 flex items-center justify-center gap-2.5">
              <span className="inline-block w-1.5 h-1.5 rounded-full bg-[#D4AF37] animate-pulse" />
              <p className="text-xs text-text-muted italic">
                {t('briefing.coldBootScanning', "Reading today's sources for new intelligence…")}
              </p>
            </div>
          ) : (
            <>
              {snapshot.synthesis && (
                <div className="pb-3 mb-1 border-b border-border">
                  <h3 className="text-[9px] font-semibold tracking-[0.1em] text-[#D4AF37] uppercase mb-2">
                    {t('briefing.synthesis', 'Synthesis')}
                  </h3>
                  {(() => {
                    const provenanceMatch = snapshot.synthesis?.match(/^([\s\S]*?)\n\n(\(\d+ signals across .+\))$/);
                    if (provenanceMatch) {
                      return (
                        <>
                          <p className="text-xs text-text-secondary leading-relaxed whitespace-pre-wrap">{provenanceMatch[1]}</p>
                          <p className="text-[9px] font-mono text-text-muted/60 mt-1.5">{provenanceMatch[2]}</p>
                        </>
                      );
                    }
                    return (
                      <p className="text-xs text-text-secondary leading-relaxed whitespace-pre-wrap">
                        {snapshot.synthesis}
                      </p>
                    );
                  })()}
                </div>
              )}
              <div>
                <h3 className="text-[9px] font-semibold tracking-[0.1em] text-text-muted uppercase mb-2">
                  {t('briefing.sourceItems', 'Source items')}
                </h3>
                <div className="space-y-2">
                  {snapshot.items.slice(0, 8).map((item) => (
                    <a
                      key={`${item.sourceType}:${item.title}`}
                      href={item.url && isSafeUrl(item.url) ? item.url : '#'}
                      target={item.url && isSafeUrl(item.url) ? '_blank' : undefined}
                      rel={item.url && isSafeUrl(item.url) ? 'noopener noreferrer' : undefined}
                      className="block pl-2 border-l-2 border-border hover:border-[#D4AF37] py-1 transition-colors"
                    >
                      <p className="text-xs text-text-primary leading-snug line-clamp-2">{item.title}</p>
                      <div className="flex items-center gap-2 mt-1">
                        <span className="text-[9px] font-mono text-text-muted uppercase tracking-wider">
                          {item.sourceType}
                        </span>
                        <span className={`text-[9px] font-medium uppercase tracking-wider ${getRelevancePresentation(item.score).colorClass}`}>
                          {t(getRelevancePresentation(item.score).labelKey)}
                        </span>
                      </div>
                    </a>
                  ))}
                </div>
              </div>
            </>
          )}
          {!isAbstention && (
            <div className="pt-2 text-[10px] text-text-muted italic">
              {t('briefing.cachedFreshening', 'Cached briefing — fresh intelligence loading…')}
            </div>
          )}
        </div>
      </div>
    </section>
  );
});
