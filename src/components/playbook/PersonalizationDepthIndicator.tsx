import type { PersonalizationDepth } from '../../types/personalization';

interface Props {
  depth: PersonalizationDepth;
}

const LEVELS = [
  { key: 'l1', label: 'Data', color: '#3B82F6' },
  { key: 'l2', label: 'Context', color: '#8B5CF6' },
  { key: 'l3', label: 'Insights', color: '#D4AF37' },
  { key: 'l4', label: 'Connections', color: '#F59E0B' },
  { key: 'l5', label: 'Evolution', color: '#22C55E' },
] as const;

function levelActive(depth: PersonalizationDepth, key: string): boolean {
  switch (key) {
    case 'l1': return depth.l1_resolved > 0;
    case 'l2': return depth.l2_evaluated > 0;
    case 'l3': return depth.l3_cards > 0;
    case 'l4': return depth.l4_connections > 0;
    case 'l5': return depth.l5_temporal > 0;
    default: return false;
  }
}

function levelCount(depth: PersonalizationDepth, key: string): number {
  switch (key) {
    case 'l1': return depth.l1_resolved;
    case 'l2': return depth.l2_evaluated;
    case 'l3': return depth.l3_cards;
    case 'l4': return depth.l4_connections;
    case 'l5': return depth.l5_temporal;
    default: return 0;
  }
}

export function PersonalizationDepthIndicator({ depth }: Props) {
  const activeCount = LEVELS.filter((l) => levelActive(depth, l.key)).length;
  if (activeCount === 0) return null;

  return (
    <div className="flex items-center gap-1.5" title={`${activeCount}/5 personalization levels active`}>
      {LEVELS.map((level) => {
        const active = levelActive(depth, level.key);
        const count = levelCount(depth, level.key);
        return (
          <div
            key={level.key}
            className="group relative"
            title={`${level.label}: ${active ? `${count} active` : 'inactive'}`}
          >
            <div
              className="w-1.5 h-3 rounded-full transition-all"
              style={{
                backgroundColor: active ? level.color : '#2A2A2A',
                opacity: active ? 1 : 0.3,
              }}
            />
          </div>
        );
      })}
      {depth.llm_pending && (
        <div className="w-1 h-1 rounded-full bg-accent-gold animate-pulse ms-0.5" title="LLM upgrade pending" />
      )}
    </div>
  );
}
