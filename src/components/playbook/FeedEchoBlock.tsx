import type { TemporalBlock, FeedEchoItem } from '../../types/personalization';

interface Props {
  block: TemporalBlock;
}

export function FeedEchoBlock({ block }: Props) {
  if (block.block_type.type !== 'feed_echo') return null;
  const { items } = block.block_type;
  if (items.length === 0) return null;

  return (
    <div className="border border-[#2A2A2A] rounded-xl bg-[#141414] p-4 my-4">
      <div className="flex items-center gap-2 mb-3">
        <div className="w-1.5 h-1.5 rounded-full bg-[#D4AF37] animate-pulse" />
        <h4 className="text-xs font-semibold text-[#A0A0A0] uppercase tracking-wider">
          Feed Signals Since Last Read
        </h4>
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
  return (
    <div className="flex items-start gap-3 py-1.5">
      <div className="w-1 h-1 rounded-full bg-[#D4AF37] mt-1.5 flex-shrink-0" />
      <div className="flex-1 min-w-0">
        {item.url ? (
          <a
            href={item.url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs text-[#A0A0A0] hover:text-[#D4AF37] transition-colors line-clamp-1"
          >
            {item.title}
          </a>
        ) : (
          <span className="text-xs text-[#A0A0A0] line-clamp-1">{item.title}</span>
        )}
        <div className="flex items-center gap-2 mt-0.5">
          <span className="text-[10px] text-[#666]">{item.source}</span>
          {item.matched_topic && (
            <span className="text-[10px] text-[#D4AF37]">#{item.matched_topic}</span>
          )}
        </div>
      </div>
    </div>
  );
}
