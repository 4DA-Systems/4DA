// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// The relevance-sorted reading feed as two collapsed lanes (see
// signals/signal-lanes.ts for the partition and ordering rules). Lane 1,
// "Your stack", is the deterministic stack-change stream rendered above this
// list by StackChangeLane. Headings are real headings, every disclosure is a
// button with aria-expanded/aria-controls, and the only counts shown are on
// controls whose action they describe ("Show 34 more").
import { useTranslation } from 'react-i18next';
import { ResultLaneList, type ResultLaneListProps } from './ResultLaneList';
import type { SignalLanes as Lanes } from './signals/signal-lanes';

type SharedListProps = Omit<ResultLaneListProps, 'id' | 'items' | 'indexOffset' | 'labelledBy' | 'ariaLabel' | 'renderPrefix'>;

interface SignalLanesProps extends SharedListProps {
  lanes: Lanes;
  visible: Lanes;
  worthExpanded: boolean;
  moreExpanded: boolean;
  onToggleWorth: () => void;
  onToggleMore: () => void;
}

const toggleClass =
  'w-full mt-1 mb-4 px-3 py-2 text-xs font-medium rounded-lg border border-border text-text-secondary ' +
  'hover:text-text-primary hover:bg-bg-tertiary focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-gold/50 transition-colors';

function LaneHeading({ id, icon, label, sub, color, border }: {
  id: string; icon: string; label: string; sub?: string; color: string; border: string;
}) {
  return (
    <div className={`flex items-baseline gap-2 mb-3 mt-4 first:mt-0 pb-1 border-b ${border}`}>
      <span aria-hidden="true">{icon}</span>
      <h3 id={id} className={`text-sm font-medium ${color}`}>{label}</h3>
      {sub && <span className="text-[10px] text-text-muted ms-1 hidden sm:inline">· {sub}</span>}
    </div>
  );
}

export function SignalLanes({
  lanes, visible, worthExpanded, moreExpanded, onToggleWorth, onToggleMore, ...shared
}: SignalLanesProps) {
  const { t } = useTranslation();
  const moreOffset = visible.worth.length;

  if (lanes.worth.length === 0 && lanes.more.length === 0) {
    return <p className="text-xs text-text-muted px-1 py-2">{t('signals.stack.feedOnlyStack')}</p>;
  }

  return (
    <div>
      {lanes.worth.length > 0 && (
        <section aria-labelledby="signal-lane-worth-heading" data-lane="worth">
          {worthExpanded ? (
            <LaneHeading
              id="signal-lane-worth-heading"
              icon="🛰"
              label={t('signals.laneWorth')}
              sub={t('signals.laneWorthSub')}
              color="text-blue-400"
              border="border-blue-500/30"
            />
          ) : (
            <h3 id="signal-lane-worth-heading" className="sr-only">{t('signals.laneWorth')}</h3>
          )}
          <button
            type="button"
            className={toggleClass}
            aria-expanded={worthExpanded}
            aria-controls={worthExpanded ? 'signal-lane-worth-list' : undefined}
            onClick={onToggleWorth}
          >
            {worthExpanded
              ? t('signals.laneHideWorth')
              : t('signals.laneShowWorth', { count: lanes.worth.length })}
          </button>
          {worthExpanded && (
            <ResultLaneList
              {...shared}
              id="signal-lane-worth-list"
              items={visible.worth}
              indexOffset={0}
              labelledBy="signal-lane-worth-heading"
            />
          )}
        </section>
      )}

      {lanes.more.length > 0 && (
        <section aria-labelledby="signal-lane-more-heading" data-lane="more">
          {moreExpanded && (
            <LaneHeading
              id="signal-lane-more-heading"
              icon="🌫"
              label={t('signals.laneMore')}
              sub={t('signals.laneMoreSub')}
              color="text-text-muted"
              border="border-border"
            />
          )}
          {!moreExpanded && <h3 id="signal-lane-more-heading" className="sr-only">{t('signals.laneMore')}</h3>}
          <button
            type="button"
            className={toggleClass}
            aria-expanded={moreExpanded}
            aria-controls={moreExpanded ? 'signal-lane-more-list' : undefined}
            onClick={onToggleMore}
          >
            {moreExpanded
              ? t('signals.laneHideMore')
              : t('signals.laneShowMore', { count: lanes.more.length })}
          </button>
          {moreExpanded && (
            <ResultLaneList
              {...shared}
              id="signal-lane-more-list"
              items={visible.more}
              indexOffset={moreOffset}
              labelledBy="signal-lane-more-heading"
            />
          )}
        </section>
      )}
    </div>
  );
}
