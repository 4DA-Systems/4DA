// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useTranslatedContent } from '../ContentTranslationProvider';
import type { ChannelSummary } from '../../types/channels';

interface Props {
  channel: ChannelSummary;
  active: boolean;
  onClick: () => void;
}

export const ChannelCard = memo(function ChannelCard({ channel, active, onClick }: Props) {
  const { t } = useTranslation();
  const { getTranslated, requestTranslation } = useTranslatedContent();

  useEffect(() => {
    const items = [{ id: `ch-title-${channel.id}`, text: channel.title }];
    if (channel.description) items.push({ id: `ch-desc-${channel.id}`, text: channel.description });
    requestTranslation(items);
  }, [channel.id, channel.title, channel.description, requestTranslation]);

  const freshnessConfig = {
    fresh: { dot: 'bg-green-500', label: t('channels.freshness.fresh') },
    stale: { dot: 'bg-amber-500', label: t('channels.freshness.stale') },
    never_rendered: {
      dot: 'bg-gray-500',
      label: t('channels.freshness.never'),
    },
  };
  const freshness = freshnessConfig[channel.freshness];

  const timeAgo = channel.last_rendered_at
    ? formatTimeAgo(channel.last_rendered_at, t)
    : t('channels.neverRendered');

  return (
    <button
      onClick={onClick}
      className={`w-full text-start p-3 rounded-lg border transition-all ${
        active
          ? 'bg-bg-secondary border-cyan-500/50 border-s-2 border-s-cyan-500'
          : 'bg-bg-secondary border-border hover:border-[#3A3A3A]'
      }`}
      aria-current={active ? 'true' : undefined}
    >
      <div className="flex items-start justify-between gap-2">
        <h3
          className={`text-sm font-medium leading-tight ${
            active ? 'text-white' : 'text-text-secondary'
          }`}
        >
          {getTranslated(`ch-title-${channel.id}`, channel.title)}
        </h3>
        <div className="flex items-center gap-1.5 flex-shrink-0">
          <div className={`w-1.5 h-1.5 rounded-full ${freshness.dot}`} />
          <span className="text-[10px] text-text-muted">
            {freshness.label}
          </span>
        </div>
      </div>
      <p className="text-xs text-text-muted mt-1 line-clamp-2">
        {getTranslated(`ch-desc-${channel.id}`, channel.description)}
      </p>
      <div className="flex items-center gap-3 mt-2">
        <span className="text-[10px] text-text-muted">
          {channel.source_count} {t('channels.sources')}
        </span>
        <span className="text-[10px] text-text-muted">{timeAgo}</span>
      </div>
    </button>
  );
});

function formatTimeAgo(dateStr: string, t: (key: string, opts?: Record<string, unknown>) => string): string {
  const date = new Date(dateStr + 'Z');
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  if (diffMins < 60) return t('channels.timeAgo.minutes', { count: diffMins });
  const diffHours = Math.floor(diffMins / 60);
  if (diffHours < 24) return t('channels.timeAgo.hours', { count: diffHours });
  const diffDays = Math.floor(diffHours / 24);
  return t('channels.timeAgo.days', { count: diffDays });
}
