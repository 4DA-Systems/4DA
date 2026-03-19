import type { TemporalBlock } from '../../types/personalization';

interface Props {
  block: TemporalBlock;
}

export function DiffRibbon({ block }: Props) {
  if (block.block_type.type !== 'diff_ribbon') return null;
  const { added, removed, changed } = block.block_type;

  if (added.length === 0 && removed.length === 0 && changed.length === 0) return null;

  return (
    <div className="border border-[#D4AF37]/20 rounded-xl bg-bg-secondary p-4 mb-4">
      <div className="flex items-center gap-2 mb-3">
        <div className="w-1.5 h-1.5 rounded-full bg-[#D4AF37] animate-pulse" />
        <h4 className="text-xs font-semibold text-[#D4AF37] uppercase tracking-wider">
          Profile Changes Since Last Read
        </h4>
      </div>
      <div className="space-y-1">
        {added.map((item, i) => (
          <div key={`a-${i}`} className="flex items-center gap-2 text-xs">
            <span className="text-[#22C55E] font-mono font-bold">+</span>
            <span className="text-[#22C55E]">{item}</span>
          </div>
        ))}
        {removed.map((item, i) => (
          <div key={`r-${i}`} className="flex items-center gap-2 text-xs">
            <span className="text-[#EF4444] font-mono font-bold">-</span>
            <span className="text-[#EF4444]">{item}</span>
          </div>
        ))}
        {changed.map((ch, i) => (
          <div key={`c-${i}`} className="flex items-center gap-2 text-xs">
            <span className="text-[#D4AF37] font-mono font-bold">~</span>
            <span className="text-text-secondary">{ch.field}:</span>
            <span className="text-[#EF4444] line-through text-[10px]">{ch.old_value}</span>
            <span className="text-[#8A8A8A]">→</span>
            <span className="text-[#22C55E]">{ch.new_value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
