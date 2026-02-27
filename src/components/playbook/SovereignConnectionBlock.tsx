import { useState } from 'react';
import type { MirrorBlock } from '../../types/personalization';

interface Props {
  block: MirrorBlock;
}

const CONNECTION_ICONS: Record<string, string> = {
  blind_spot_moat: '⊕',
  feed_predicts_engine: '◎',
  radar_momentum: '↗',
};

export function SovereignConnectionBlock({ block }: Props) {
  const [expanded, setExpanded] = useState(false);
  const icon = CONNECTION_ICONS[block.connection_type] ?? '◆';

  return (
    <div className="border border-[#D4AF37]/10 rounded-xl bg-bg-primary p-4 my-4">
      <div className="flex items-start gap-3">
        <span className="text-lg text-[#D4AF37] leading-none mt-0.5">{icon}</span>
        <div className="flex-1 min-w-0">
          <h4 className="text-sm font-medium text-white mb-1">{block.headline}</h4>
          <p className="text-xs text-text-secondary leading-relaxed">{block.insight}</p>
        </div>
      </div>

      {/* Collapsible data sources panel (Sovereign Receipt) */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="mt-3 text-[10px] text-[#666] hover:text-text-secondary transition-colors"
      >
        {expanded ? '▾' : '▸'} Data Sources ({block.data_sources.length})
      </button>
      {expanded && (
        <div className="mt-2 flex gap-1.5 flex-wrap">
          {block.data_sources.map((src) => (
            <span key={src} className="px-2 py-0.5 bg-bg-tertiary text-[10px] text-text-secondary rounded">
              {src}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
