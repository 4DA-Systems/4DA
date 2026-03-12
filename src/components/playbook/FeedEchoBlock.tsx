import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { TemporalBlock, FeedEchoItem } from '../../types/personalization';

interface Props {
  block: TemporalBlock;
}

export function FeedEchoBlock({ block }: Props) {
  const { t } = useTranslation();

  if (block.block_type.type !== 'feed_echo') return null;
  const { items } = block.block_type;
  if (items.length === 0) return null;

  // Count items fetched in the last 24 hours as "new"
  const newCount = items.filter((item) => {
    try {
      const fetched = new Date(item.fetched_at).getTime();
      return Date.now() - fetched < 24 * 60 * 60 * 1000;
    } catch {
      return false;
    }
  }).length;

  return (
    <div className="border border-[#D4AF37]/20 rounded-xl bg-[#141414] p-4 my-4">
      <div className="flex items-center gap-2 mb-3">
        <div className="w-1.5 h-1.5 rounded-full bg-[#D4AF37] animate-pulse" />
        <h4 className="text-xs font-semibold text-text-secondary uppercase tracking-wider">
          Feed Signals Since Last Read
        </h4>
        {newCount > 0 && (
          <span className="ml-auto px-1.5 py-0.5 text-[10px] font-medium bg-[#D4AF37]/15 text-[#D4AF37] rounded-full">
            {t('playbook.feedEchoNewItems', { count: newCount })}
          </span>
        )}
      </div>
      <div className="space-y-2">
        {items.map((item, i) => (
          <FeedEchoRow key={i} item={item} />
        ))}
      </div>
    </div>
  );
}

function FeedEchoRow({ item }: { item: FeedEchoItem }) {
  const { t } = useTranslation();
  const [saved, setSaved] = useState(false);

  const handleSave = useCallback(() => {
    // Copy the item URL or title to clipboard as a lightweight save
    const text = item.url || item.title;
    navigator.clipboard.writeText(text).then(() => {
      setSaved(true);
    }).catch(() => {
      // Fallback: just mark as saved visually
      setSaved(true);
    });
  }, [item.url, item.title]);

  return (
    <div className="flex items-start gap-3 py-1.5 group">
      <div className="w-1 h-1 rounded-full bg-[#D4AF37] mt-1.5 flex-shrink-0" />
      <div className="flex-1 min-w-0">
        {item.url ? (
          <a
            href={item.url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs text-text-secondary hover:text-[#D4AF37] transition-colors line-clamp-1"
          >
            {item.title}
          </a>
        ) : (
          <span className="text-xs text-text-secondary line-clamp-1">{item.title}</span>
        )}
        <div className="flex items-center gap-2 mt-0.5">
          <span className="text-[10px] text-[#666]">{item.source}</span>
          {item.matched_topic && (
            <span className="text-[10px] text-[#D4AF37]">#{item.matched_topic}</span>
          )}
        </div>
      </div>
      <button
        onClick={handleSave}
        disabled={saved}
        className={`flex-shrink-0 px-2 py-0.5 text-[10px] rounded transition-all ${
          saved
            ? 'bg-[#22C55E]/15 text-[#22C55E] cursor-default'
            : 'bg-[#1F1F1F] text-[#787878] hover:text-[#D4AF37] hover:bg-[#D4AF37]/10 opacity-0 group-hover:opacity-100'
        }`}
        aria-label={saved ? t('playbook.feedEchoSaved') : t('playbook.feedEchoSave')}
      >
        {saved ? t('playbook.feedEchoSaved') : t('playbook.feedEchoSave')}
      </button>
    </div>
  );
}
