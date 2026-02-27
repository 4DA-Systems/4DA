import type { TemporalBlock } from '../../types/personalization';

interface Props {
  block: TemporalBlock;
}

export function ProgressiveRevealBanner({ block }: Props) {
  if (block.block_type.type !== 'progressive_reveal') return null;
  const { newly_completed, unlocked_content } = block.block_type;
  if (newly_completed.length === 0) return null;

  return (
    <div className="border border-[#D4AF37]/30 rounded-xl bg-[#D4AF37]/5 p-4 mb-4">
      <div className="flex items-center gap-2 mb-2">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#D4AF37" strokeWidth="2">
          <path d="M12 2L2 7l10 5 10-5-10-5z" />
          <path d="M2 17l10 5 10-5" />
          <path d="M2 12l10 5 10-5" />
        </svg>
        <h4 className="text-xs font-semibold text-[#D4AF37] uppercase tracking-wider">
          New Data Unlocked
        </h4>
      </div>
      <p className="text-xs text-text-secondary mb-2">
        Completed modules: {newly_completed.join(', ')}
      </p>
      <ul className="space-y-1">
        {unlocked_content.map((item, i) => (
          <li key={i} className="flex items-center gap-2 text-xs text-text-secondary">
            <span className="text-[#22C55E]">+</span>
            {item}
          </li>
        ))}
      </ul>
    </div>
  );
}
