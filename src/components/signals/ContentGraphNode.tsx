// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import { Handle, Position, type NodeProps, type Node } from '@xyflow/react';

interface ContentNodeData {
  title: string;
  url: string | null;
  source_type: string;
  relevance_score: number;
  signal_type: string | null;
  signal_priority: string | null;
  primary_topic: string | null;
  cluster_id: string | null;
  isNew?: boolean;
  [key: string]: unknown;
}

export type ContentNode = Node<ContentNodeData, 'contentNode'>;

const SOURCE_COLORS: Record<string, string> = {
  hackernews: '#F97316',
  reddit: '#3B82F6',
  github: '#6B7280',
  arxiv: '#A855F7',
  rss: '#D97706',
  devto: '#22C55E',
  lobsters: '#EF4444',
  lemmy: '#22C55E',
  mastodon: '#A855F7',
  bluesky: '#3B82F6',
  producthunt: '#F97316',
  crates_io: '#F97316',
  npm: '#EF4444',
  pypi: '#3B82F6',
  youtube: '#EF4444',
  stackoverflow: '#F97316',
  twitter: '#0EA5E9',
  huggingface: '#EAB308',
  cve: '#EF4444',
  osv: '#EF4444',
  papers_with_code: '#6366F1',
  go_modules: '#06B6D4',
};

export { SOURCE_COLORS };

function getGlowStyle(priority: string | null): string {
  if (priority === 'critical') return '0 0 12px 3px rgba(239, 68, 68, 0.5)';
  if (priority === 'alert') return '0 0 10px 2px rgba(249, 115, 22, 0.4)';
  return 'none';
}

function brighten(hex: string): string {
  const r = Math.min(255, parseInt(hex.slice(1, 3), 16) + 40);
  const g = Math.min(255, parseInt(hex.slice(3, 5), 16) + 40);
  const b = Math.min(255, parseInt(hex.slice(5, 7), 16) + 40);
  return `rgb(${r}, ${g}, ${b})`;
}

function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  return text.slice(0, max - 1) + '…';
}

const ContentGraphNode = memo(function ContentGraphNode({ data }: NodeProps<ContentNode>) {
  const [hovered, setHovered] = useState(false);
  const onEnter = useCallback(() => setHovered(true), []);
  const onLeave = useCallback(() => setHovered(false), []);

  const color = SOURCE_COLORS[data.source_type] ?? '#6B7280';
  const size = 28 + (data.relevance_score * 28);
  const glow = getGlowStyle(data.signal_priority);

  return (
    <div
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      style={{ position: 'relative' }}
    >
      <Handle
        type="target"
        position={Position.Top}
        style={{ width: 0, height: 0, border: 'none', background: 'transparent' }}
      />

      {data.isNew && (
        <div
          style={{
            position: 'absolute',
            inset: -4,
            borderRadius: '50%',
            border: `2px solid ${color}`,
            opacity: 0.6,
            animation: 'graph-node-pulse 2s ease-in-out infinite',
          }}
        />
      )}

      <div
        style={{
          width: size,
          height: size,
          borderRadius: '50%',
          backgroundColor: color,
          border: `2px solid ${brighten(color)}`,
          boxShadow: glow,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          cursor: 'pointer',
          transition: 'transform 150ms ease',
          transform: hovered ? 'scale(1.15)' : 'scale(1)',
        }}
      >
        <span
          style={{
            color: 'var(--color-text-primary)',
            fontSize: Math.max(8, size * 0.22),
            fontFamily: 'Inter, sans-serif',
            fontWeight: 500,
            lineHeight: 1.1,
            textAlign: 'center',
            padding: 2,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
            maxWidth: size - 8,
          }}
        >
          {truncate(data.title, 25)}
        </span>
      </div>

      {hovered && (
        <div
          style={{
            position: 'absolute',
            top: size + 6,
            left: '50%',
            transform: 'translateX(-50%)',
            backgroundColor: 'var(--color-bg-tertiary)',
            border: '1px solid var(--color-border)',
            borderRadius: 6,
            padding: '8px 10px',
            zIndex: 50,
            minWidth: 180,
            maxWidth: 280,
            pointerEvents: 'none',
          }}
        >
          <div style={{ color: 'var(--color-text-primary)', fontSize: 12, fontWeight: 600, marginBottom: 4, fontFamily: 'Inter, sans-serif' }}>
            {data.title}
          </div>
          <div style={{ color: 'var(--color-text-secondary)', fontSize: 11, fontFamily: 'Inter, sans-serif' }}>
            {data.source_type}
            {data.signal_type && ` · ${data.signal_type}`}
          </div>
          {data.primary_topic && (
            <div style={{ color: 'var(--color-text-muted)', fontSize: 10, marginTop: 2, fontFamily: 'Inter, sans-serif' }}>
              {data.primary_topic}
            </div>
          )}
        </div>
      )}

      <Handle
        type="source"
        position={Position.Bottom}
        style={{ width: 0, height: 0, border: 'none', background: 'transparent' }}
      />
    </div>
  );
});

export default ContentGraphNode;
