import type { TemporalBlock } from '../../types/personalization';

interface Props {
  block: TemporalBlock;
}

export function DiffRibbon({ block }: Props) {
  if (block.block_type.type !== 'diff_ribbon') return null;
  const { added, removed, changed } = block.block_type;

  if (added.length === 0 && removed.length === 0 && changed.length === 0) return null;

  return (
    <div className="border border-accent-gold/20 rounded-xl bg-bg-secondary p-4 mb-4">
      <div className="flex items-center gap-2 mb-3">
        <div className="w-1.5 h-1.5 rounded-full bg-accent-gold animate-pulse" />
        <h4 className="text-xs font-semibold text-accent-gold uppercase tracking-wider">
          Profile Changes Since Last Read
        </h4>
      </div>
      <div className="space-y-1">
        {added.map((item, i) => (
          <div key={`a-${i}`} className="flex items-center gap-2 text-xs">
            <span className="text-success font-mono font-bold">+</span>
            <span className="text-success">{item}</span>
          </div>
        ))}
        {removed.map((item, i) => (
          <div key={`r-${i}`} className="flex items-center gap-2 text-xs">
            <span className="text-error font-mono font-bold">-</span>
            <span className="text-error">{item}</span>
          </div>
        ))}
        {changed.map((ch, i) => (
          <div key={`c-${i}`} className="flex items-center gap-2 text-xs">
            <span className="text-accent-gold font-mono font-bold">~</span>
            <span className="text-text-secondary">{ch.field}:</span>
            <span className="text-error line-through text-[10px]">{ch.old_value}</span>
            <span className="text-text-muted">→</span>
            <span className="text-success">{ch.new_value}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
