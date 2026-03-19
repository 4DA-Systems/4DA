import type { Visualization } from '../../types/personalization';

interface Props {
  viz: Visualization;
}

export function VisualizationRenderer({ viz }: Props) {
  switch (viz.type) {
    case 'bar_chart':
      return <BarChart bars={viz.bars} maxValue={viz.max_value} unit={viz.unit} />;
    case 'rank_list':
      return <RankList items={viz.items} />;
    case 't_shape':
      return <TShapeDiagram primary={viz.primary} depthLabel={viz.depth_label} adjacent={viz.adjacent} breadthLabel={viz.breadth_label} />;
    case 'rate_table':
      return <RateTable headers={viz.headers} rows={viz.rows} />;
    case 'diff_ribbon':
      return <DiffRibbonViz added={viz.added} removed={viz.removed} changed={viz.changed} />;
    default:
      return null;
  }
}

function BarChart({ bars, maxValue, unit }: { bars: { label: string; value: number; highlight: boolean }[]; maxValue: number; unit: string }) {
  return (
    <div className="space-y-2">
      {bars.map((bar) => {
        const pct = maxValue > 0 ? (bar.value / maxValue) * 100 : 0;
        return (
          <div key={bar.label} className="flex items-center gap-3">
            <span className="text-[10px] text-[#8A8A8A] w-12 text-right flex-shrink-0">{bar.label}</span>
            <div className="flex-1 h-4 bg-bg-tertiary rounded-full overflow-hidden relative">
              <div
                className="h-full rounded-full transition-all duration-500"
                style={{
                  width: `${Math.min(pct, 100)}%`,
                  backgroundColor: bar.highlight ? '#D4AF37' : '#3B82F6',
                }}
              />
            </div>
            <span className={`text-xs flex-shrink-0 w-16 text-right ${bar.highlight ? 'text-[#D4AF37] font-medium' : 'text-text-secondary'}`}>
              {bar.value.toFixed(0)} {unit}
            </span>
          </div>
        );
      })}
      {/* Max reference line label */}
      <div className="flex justify-end">
        <span className="text-[10px] text-[#8A8A8A]">max: {maxValue} {unit}</span>
      </div>
    </div>
  );
}

function RankList({ items }: { items: { rank: number; name: string; score: number; matches_stack: boolean }[] }) {
  return (
    <div className="space-y-1.5">
      {items.map((item) => (
        <div key={item.rank} className="flex items-center gap-2">
          <span className="text-[10px] text-[#8A8A8A] w-5 text-center flex-shrink-0">#{item.rank}</span>
          <span className={`text-xs flex-1 ${item.matches_stack ? 'text-white' : 'text-text-secondary'}`}>
            {item.name}
          </span>
          <span className="flex-shrink-0">
            {item.matches_stack ? (
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#22C55E" strokeWidth="2"><polyline points="20 6 9 17 4 12" /></svg>
            ) : (
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#8A8A8A" strokeWidth="2"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
            )}
          </span>
          <span className="text-[10px] text-[#8A8A8A] w-10 text-right flex-shrink-0">
            {Math.round(item.score * 100)}%
          </span>
        </div>
      ))}
    </div>
  );
}

function TShapeDiagram({ primary, depthLabel, adjacent, breadthLabel }: {
  primary: string; depthLabel: string; adjacent: string[]; breadthLabel: string;
}) {
  return (
    <div className="flex flex-col items-center gap-2 py-2">
      {/* Breadth bar */}
      <div className="flex items-center gap-1 flex-wrap justify-center">
        {adjacent.map((tech) => (
          <span key={tech} className="px-2 py-0.5 bg-bg-tertiary border border-border text-[10px] text-text-secondary rounded">
            {tech}
          </span>
        ))}
      </div>
      <span className="text-[10px] text-[#8A8A8A]">{breadthLabel}</span>

      {/* T-connector */}
      <div className="w-px h-3 bg-[#D4AF37]" />

      {/* Depth column */}
      <div className="flex flex-col items-center gap-1">
        <span className="px-3 py-1 bg-[#D4AF37]/20 border border-[#D4AF37]/40 text-xs text-[#D4AF37] font-medium rounded">
          {primary}
        </span>
        <span className="text-[10px] text-[#8A8A8A]">{depthLabel}</span>
      </div>
    </div>
  );
}

function RateTable({ headers, rows }: { headers: string[]; rows: { cells: string[]; highlight: boolean }[] }) {
  return (
    <table className="w-full text-xs border border-border rounded">
      <thead>
        <tr className="bg-bg-tertiary">
          {headers.map((h, i) => (
            <th key={i} className="px-3 py-1.5 text-left text-white font-medium border-b border-border">{h}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, ri) => (
          <tr key={ri} className={`border-b border-border last:border-0 ${row.highlight ? 'bg-[#D4AF37]/5' : ''}`}>
            {row.cells.map((cell, ci) => (
              <td key={ci} className={`px-3 py-1.5 ${row.highlight ? 'text-[#D4AF37]' : 'text-text-secondary'}`}>{cell}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function DiffRibbonViz({ added, removed, changed }: {
  added: string[]; removed: string[]; changed: { field: string; old_value: string; new_value: string }[];
}) {
  return (
    <div className="space-y-1">
      {added.map((item, i) => (
        <div key={`a-${i}`} className="flex items-center gap-2 text-xs">
          <span className="text-[#22C55E] font-mono">+</span>
          <span className="text-[#22C55E]">{item}</span>
        </div>
      ))}
      {removed.map((item, i) => (
        <div key={`r-${i}`} className="flex items-center gap-2 text-xs">
          <span className="text-[#EF4444] font-mono">-</span>
          <span className="text-[#EF4444]">{item}</span>
        </div>
      ))}
      {changed.map((ch, i) => (
        <div key={`c-${i}`} className="flex items-center gap-2 text-xs">
          <span className="text-[#D4AF37] font-mono">~</span>
          <span className="text-text-secondary">{ch.field}:</span>
          <span className="text-[#EF4444] line-through">{ch.old_value}</span>
          <span className="text-text-secondary">→</span>
          <span className="text-[#22C55E]">{ch.new_value}</span>
        </div>
      ))}
    </div>
  );
}
