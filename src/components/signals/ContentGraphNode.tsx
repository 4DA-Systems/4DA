// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useState, useCallback } from 'react';
import { Handle, Position, type NodeProps, type Node } from '@xyflow/react';
import { useTranslation } from 'react-i18next';
// Theme overrides for React Flow's chrome (zoom Controls, MiniMap). Imported
// here — the node module is always in the graph bundle — so it also applies to
// the Controls/MiniMap rendered by ContentGraphView without editing that file.
import './content-graph.css';

interface ContentNodeData {
  title: string;
  url: string | null;
  source_type: string;
  relevance_score: number;
  signal_type: string | null;
  signal_priority: string | null;
  primary_topic: string | null;
  cluster_id: string | null;
  /** Items this node represents; >1 = a story of collapsed near-duplicates. */
  member_count: number;
  /** Item ids of ALL members including the representative — detail panel hydration. */
  member_ids: number[];
  /** Content category — the primary color + shape channel. */
  category: string;
  /** Linked to the user's declared dependencies → gold ring. */
  affects_you: boolean;
  isNew?: boolean;
  [key: string]: unknown;
}

export type ContentNode = Node<ContentNodeData, 'contentNode'>;

// Category is the color channel (source identity lives in the tooltip — 22
// sources shared 8 hues, five of them one red; undecodable by construction).
// Palette validated with the dataviz six-checks validator on the #0A0A0A
// surface: CVD-adjacent ΔE 9.2 (deutan), normal-vision 21.6, contrast ≥3:1.
// Each category ALSO carries a distinct silhouette (shape), so identity never
// rides on hue alone (colorblind/grayscale-safe).
const CATEGORY_COLORS: Record<string, string> = {
  security: '#C23237',
  release: '#CFA01F',
  discussion: '#3B9EFF',
  research: '#B658C4',
};
const DEFAULT_CATEGORY_COLOR = '#6B7280';

/** Gold = "touches your declared stack" (reserved accent, never a category). */
const AFFECTS_GOLD = '#D4AF37';

interface CategoryShape {
  borderRadius: string;
  rotate: boolean;
  donut: boolean;
}

// Distinct silhouettes: circle (discussion), rounded square (release),
// diamond (security — reads as an alert marker), donut (research). All are
// border-radius/rotation based so box-shadow rings follow the shape.
const CATEGORY_SHAPES: Record<string, CategoryShape> = {
  discussion: { borderRadius: '50%', rotate: false, donut: false },
  release: { borderRadius: '22%', rotate: false, donut: false },
  security: { borderRadius: '18%', rotate: true, donut: false },
  research: { borderRadius: '50%', rotate: false, donut: true },
};
const DEFAULT_SHAPE: CategoryShape = CATEGORY_SHAPES.discussion!;

export { CATEGORY_COLORS, AFFECTS_GOLD, CATEGORY_SHAPES };

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

// The node color already encodes the source, so a redundant "crates.io: " /
// "npm: " prefix just eats label space. Strip a leading KNOWN-source prefix
// only — never a generic "word:" so real titles like "Rust 1.80: released"
// keep their colon.
const SOURCE_PREFIX =
  /^(crates\.io|npm|pypi|pep|github|gh|hn|reddit|arxiv|dev\.to|lobsters|lobste\.rs|stack ?overflow|so|product ?hunt|hugging ?face|hf|go modules?|youtube|yt|bluesky|mastodon|cve|osv|rss)\s*[:\-–]\s+/i;

function cleanTitle(raw: string): string {
  return raw.replace(SOURCE_PREFIX, '').trim() || raw;
}

const ContentGraphNode = memo(function ContentGraphNode({ data, selected }: NodeProps<ContentNode>) {
  const { t } = useTranslation();
  const [hovered, setHovered] = useState(false);
  const onEnter = useCallback(() => setHovered(true), []);
  const onLeave = useCallback(() => setHovered(false), []);

  const color = CATEGORY_COLORS[data.category] ?? DEFAULT_CATEGORY_COLOR;
  const shape = CATEGORY_SHAPES[data.category] ?? DEFAULT_SHAPE;
  const memberCount = data.member_count ?? 1;
  // Stories grow with how much they collapsed (sqrt: 26 advisories shouldn't
  // be 26x the dot); plain items keep the relevance sizing.
  const size =
    memberCount > 1
      ? Math.min(72, 36 + Math.sqrt(memberCount) * 6)
      : 28 + data.relevance_score * 28;
  const glow = getGlowStyle(data.signal_priority);
  const label = cleanTitle(data.title);
  const extraCount = memberCount - 1;

  // Gold ring = touches your declared stack; a 2px surface-color gap keeps
  // the ring readable against every category fill (incl. the amber release).
  const affectsRing = data.affects_you
    ? `0 0 0 2px var(--color-bg-primary), 0 0 0 4px ${AFFECTS_GOLD}`
    : '';
  // Selection ring (detail panel open) sits outside every other ring so it
  // never collides with the gold stack ring.
  const selectedRing = selected
    ? `0 0 0 ${data.affects_you ? 6 : 2}px var(--color-bg-primary), 0 0 0 ${data.affects_you ? 8 : 4}px var(--color-text-primary)`
    : '';
  const boxShadow = [affectsRing, selectedRing, glow === 'none' ? '' : glow]
    .filter(Boolean)
    .join(', ') || 'none';
  const shapeTransform = shape.rotate ? ' rotate(45deg)' : '';

  return (
    <div
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
      style={{ position: 'relative', width: size, height: size }}
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
            borderRadius: shape.borderRadius,
            transform: shape.rotate ? 'rotate(45deg)' : undefined,
            border: `2px solid ${color}`,
            opacity: 0.6,
            animation: 'graph-node-pulse 2s ease-in-out infinite',
          }}
        />
      )}

      {/* The mark carries category (color + silhouette), story mass /
          relevance (size), priority (glow) and stack relevance (gold ring).
          The title lives in the readable label below — text jammed inside a
          28-56px shape was illegible. The label is absolutely positioned so
          the node's measured box stays the mark and edges keep anchoring at
          its center. */}
      <div
        style={{
          width: size,
          height: size,
          borderRadius: shape.borderRadius,
          backgroundColor: color,
          border: `2px solid ${brighten(color)}`,
          boxShadow,
          cursor: 'pointer',
          transition: 'transform 150ms ease',
          transform: (hovered ? 'scale(1.15)' : 'scale(1)') + shapeTransform,
        }}
      >
        {shape.donut && (
          <div
            style={{
              position: 'absolute',
              inset: '32%',
              borderRadius: '50%',
              backgroundColor: 'var(--color-bg-primary)',
              pointerEvents: 'none',
            }}
          />
        )}
      </div>

      {extraCount > 0 && (
        <span
          style={{
            position: 'absolute',
            top: -6,
            right: -10,
            padding: '1px 5px',
            borderRadius: 8,
            backgroundColor: 'var(--color-bg-tertiary)',
            border: `1px solid ${brighten(color)}`,
            color: 'var(--color-text-primary)',
            fontSize: 9,
            fontWeight: 600,
            fontFamily: 'JetBrains Mono, monospace',
            lineHeight: 1.4,
            pointerEvents: 'none',
          }}
        >
          {`+${extraCount}`}
        </span>
      )}

      <span
        style={{
          position: 'absolute',
          top: size + 3,
          left: '50%',
          transform: 'translateX(-50%)',
          width: 128,
          color: hovered ? 'var(--color-text-primary)' : 'var(--color-text-secondary)',
          fontSize: 10,
          fontFamily: 'Inter, sans-serif',
          fontWeight: 500,
          lineHeight: 1.15,
          textAlign: 'center',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          pointerEvents: 'none',
          // Halo in the page color keeps labels legible over edge lines.
          textShadow: '0 1px 4px var(--color-bg-primary), 0 0 2px var(--color-bg-primary)',
        }}
      >
        {truncate(label, 22)}
      </span>

      {hovered && (
        <div
          style={{
            position: 'absolute',
            top: size + 22,
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
          {/* Hover is for scanning; depth lives in the click-through detail
              panel, which lists every member openable. The old per-title dump
              here duplicated the panel and made 25-member advisory storms
              throw a giant hover box over the canvas. */}
          {extraCount > 0 && (
            <div
              style={{
                marginTop: 6,
                paddingTop: 6,
                borderTop: '1px solid var(--color-border)',
                color: 'var(--color-text-secondary)',
                fontSize: 10,
                fontWeight: 600,
                fontFamily: 'Inter, sans-serif',
              }}
            >
              {t('signals.graphStoryMembers', { count: extraCount })}
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
