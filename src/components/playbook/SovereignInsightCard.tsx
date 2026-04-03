import { useState, useEffect } from 'react';
import { useTranslatedContent } from '../ContentTranslationProvider';
import type { InsightBlock, DataPoint } from '../../types/personalization';
import { VisualizationRenderer } from './VisualizationRenderer';

interface Props {
  block: InsightBlock;
}

export function SovereignInsightCard({ block }: Props) {
  const [showSources, setShowSources] = useState(false);
  const { getTranslated, requestTranslation } = useTranslatedContent();

  useEffect(() => {
    const items: { id: string; text: string }[] = [];
    if (block.content.type === 'prose') {
      items.push({ id: `insight-text-${block.block_id}`, text: block.content.text });
    } else {
      items.push({ id: `insight-title-${block.block_id}`, text: block.content.title });
      block.content.data_points.forEach((dp, i) => {
        items.push({ id: `insight-dp-label-${block.block_id}-${i}`, text: dp.label });
        if (dp.context) items.push({ id: `insight-dp-ctx-${block.block_id}-${i}`, text: dp.context });
      });
    }
    if (items.length > 0) requestTranslation(items);
  }, [block, requestTranslation]);

  if (block.content.type === 'prose') {
    return (
      <div className="border border-accent-gold/20 rounded-xl bg-bg-secondary p-5 my-4">
        <p className="text-sm text-text-secondary leading-relaxed whitespace-pre-wrap">
          {getTranslated(`insight-text-${block.block_id}`, block.content.text)}
        </p>
        <SourceLabels labels={block.source_labels} show={showSources} onToggle={() => setShowSources(!showSources)} />
      </div>
    );
  }

  // Card content (no-LLM path)
  const card = block.content;
  return (
    <div className="border border-accent-gold/20 rounded-xl bg-bg-secondary overflow-hidden my-4">
      {/* Header */}
      <div className="px-5 py-3 border-b border-border flex items-center justify-between">
        <h4 className="text-sm font-semibold text-white">{getTranslated(`insight-title-${block.block_id}`, card.title)}</h4>
        <ConfidenceBar confidence={block.confidence} />
      </div>

      {/* Data Points */}
      <div className="px-5 py-4 space-y-2.5">
        {card.data_points.map((dp, i) => (
          <DataPointRow key={i} point={dp} blockId={block.block_id} index={i} getTranslated={getTranslated} />
        ))}
      </div>

      {/* Visualization */}
      {card.visualization && (
        <div className="px-5 pb-4">
          <VisualizationRenderer viz={card.visualization} />
        </div>
      )}

      {/* Source Labels (collapsible) */}
      <SourceLabels labels={block.source_labels} show={showSources} onToggle={() => setShowSources(!showSources)} />
    </div>
  );
}

function DataPointRow({ point, blockId, index, getTranslated }: {
  point: DataPoint;
  blockId: string;
  index: number;
  getTranslated: (id: string, original: string) => string;
}) {
  return (
    <div className={`flex items-start justify-between gap-4 ${point.highlight ? 'text-white' : 'text-text-secondary'}`}>
      <span className="text-xs text-text-muted flex-shrink-0 w-32">{getTranslated(`insight-dp-label-${blockId}-${index}`, point.label)}</span>
      <div className="flex-1 text-end">
        <span className={`text-sm ${point.highlight ? 'text-accent-gold font-medium' : ''}`}>
          {point.value}
        </span>
        {point.context && (
          <p className="text-[10px] text-text-muted mt-0.5">{getTranslated(`insight-dp-ctx-${blockId}-${index}`, point.context)}</p>
        )}
      </div>
    </div>
  );
}

function ConfidenceBar({ confidence }: { confidence: number }) {
  const pct = Math.round(confidence * 100);
  const color = pct >= 70 ? '#22C55E' : pct >= 40 ? '#D4AF37' : '#8A8A8A';
  return (
    <div className="flex items-center gap-1.5" title={`${pct}% data coverage`}>
      <div className="w-12 h-1.5 bg-border rounded-full overflow-hidden">
        <div className="h-full rounded-full transition-all" style={{ width: `${pct}%`, backgroundColor: color }} />
      </div>
      <span className="text-[10px] text-text-muted">{pct}%</span>
    </div>
  );
}

function SourceLabels({ labels, show, onToggle }: { labels: string[]; show: boolean; onToggle: () => void }) {
  if (labels.length === 0) return null;
  return (
    <div className="px-5 py-2 border-t border-border">
      <button onClick={onToggle} className="text-[10px] text-text-muted hover:text-text-secondary transition-colors">
        {show ? '▾' : '▸'} Data Sources ({labels.length})
      </button>
      {show && (
        <div className="flex gap-1.5 mt-1.5 flex-wrap">
          {labels.map((label) => (
            <span key={label} className="px-2 py-0.5 bg-bg-tertiary text-[10px] text-text-secondary rounded">
              {label}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
