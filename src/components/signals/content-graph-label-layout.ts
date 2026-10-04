// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Label collision avoidance for the content graph — pure geometry, no React,
// no DOM. Labels are zoom-invariant (graph-zoom.ts): their ON-SCREEN size is
// fixed below zoom 1 while node spacing shrinks with zoom, so at fit view a
// 150-node map piled stack labels and cluster headers on top of each other
// (live 2026-10-04: "tauri-plugin-noti…" over "tauri-plugin-opener…", "ARC ·
// DOWNCASTING · PINING" across "BROWSER"). Overlap depends on zoom only —
// panning translates every box equally — so the resolver runs per zoom step.

import {
  DEFAULT_OFFSET,
  resolveLabelCollisions,
  type LabelBox,
  type LabelOffset,
  type LabelPlacement,
} from './content-graph-label-collision';

export {
  DEFAULT_OFFSET,
  countOverlaps,
  rectsOverlap,
  resolveLabelCollisions,
  type LabelBox,
  type LabelOffset,
  type LabelPlacement,
} from './content-graph-label-collision';

// ---------------------------------------------------------------------------
// Label text + geometry shared with the renderers (single source of truth, so
// the boxes the resolver reasons about are the boxes that paint).

/** Node label typography (ContentGraphNode). */
export const NODE_LABEL_FONT_PX = 11;
export const NODE_LABEL_LINE = 1.15;
export const NODE_LABEL_MAX_W = 150;
export const NODE_LABEL_MAX_CHARS = 24;
/** Gap between a node mark and its label, flow units. */
export const NODE_LABEL_GAP = 3;
/** Cluster / lane header typography (ContentGraphChrome). */
export const HEADER_FONT_PX = 12;
export const HEADER_COUNT_FONT_PX = 11;
export const HEADER_LETTER_SPACING_EM = 0.03;
/** Rendered header line box (measured live 2026-10-04: 18px at 12px). */
export const HEADER_LINE = 1.5;
/** Lane header: bottom padding + dashed rule under the text, screen px. */
export const LANE_HEADER_RULE_PX = 5;
/** Free space kept between the lane header and the lane's first row, screen px. */
export const LANE_HEADER_GAP_PX = 6;
/** Minimum free space between two labels, screen px. */
export const LABEL_GAP_PX = 2;

// The node color already encodes the source, so a redundant "crates.io: " /
// "npm: " prefix just eats label space. Strip a leading KNOWN-source prefix
// only — never a generic "word:" so real titles like "Rust 1.80: released"
// keep their colon.
const SOURCE_PREFIX =
  /^(crates\.io|npm|pypi|pep|github|gh|hn|reddit|arxiv|dev\.to|lobsters|lobste\.rs|stack ?overflow|so|product ?hunt|hugging ?face|hf|go modules?|youtube|yt|bluesky|mastodon|cve|osv|rss)\s*[:\-–]\s+/i;

export function cleanTitle(raw: string): string {
  return raw.replace(SOURCE_PREFIX, '').trim() || raw;
}

export function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  return text.slice(0, max - 1) + '…';
}

/** The text a node label paints. */
export function nodeLabelText(title: string): string {
  return truncate(cleanTitle(title), NODE_LABEL_MAX_CHARS);
}

/** Node mark size: stories grow with how much they collapsed (sqrt: 26
 *  advisories shouldn't be 26x the dot); plain items size by relevance. */
export function nodeMarkSize(memberCount: number, relevance: number): number {
  return memberCount > 1 ? Math.min(72, 36 + Math.sqrt(memberCount) * 6) : 28 + relevance * 28;
}

// ---------------------------------------------------------------------------
// Box construction: graph geometry + zoom → the boxes the resolver places.

export interface NodeLabelInput {
  id: string;
  /** Top-left of the node mark, flow units (React Flow `position`). */
  x: number;
  y: number;
  title: string;
  memberCount: number;
  relevance: number;
  stack: boolean;
}

export interface HeaderLabelInput {
  id: string;
  /** Header anchor: horizontal CENTRE and top edge, flow units. */
  x: number;
  y: number;
  label: string;
  count: number;
  /** Hull radius — how far the header may travel to clear a neighbour. */
  radius: number;
}

export interface LaneLabelInput {
  /** Left edge of the lane, and the top of its first row, flow units. */
  x: number;
  y: number;
  text: string;
}

/** Width in SCREEN px of `text` at `fontPx` / `weight` (no letter spacing). */
export type MeasureText = (text: string, fontPx: number, weight: number) => number;

export interface LabelLayoutInput {
  zoom: number;
  /** False at far zoom, where content-graph.css hides non-stack labels. */
  includeNonStack: boolean;
  nodes: NodeLabelInput[];
  headers: HeaderLabelInput[];
  lane: LaneLabelInput | null;
  measure: MeasureText;
}

const PRIORITY_LANE = 1e7;
/** Cost of a label covering a stack mark (vs 0.15 per placement step). */
const WEIGHT_STACK_MARK = 3;
/** Obstacle ids: stack marks the labels keep clear of. Not labels. */
const MARK_PREFIX = 'mark:';
/** Gold stack ring outer width, screen px (ContentGraphNode stackRing). */
const STACK_RING_PX = 5;
const PRIORITY_HEADER = 1e5;
const PRIORITY_STACK = 1e3;
/** Avoidance weights: a header should sooner cover two stack labels than
 *  another header; a stack label sooner two plain ones than one stack. */
const WEIGHT_HEADER = 8;
const WEIGHT_STACK = 2;

/** Screen px → flow units for a zoom-invariant size (see graph-zoom.ts). */
export const invariantScale = (zoom: number): number => 1 / Math.min(zoom > 0 ? zoom : 1, 1);

function headerTextWidth(label: string, count: number, measure: MeasureText): number {
  const ls = HEADER_LETTER_SPACING_EM;
  const main = label.toUpperCase();
  const tail = `(${count})`;
  return (
    measure(main, HEADER_FONT_PX, 600) +
    main.length * ls * HEADER_FONT_PX +
    4 +
    measure(tail, HEADER_COUNT_FONT_PX, 400) +
    tail.length * ls * HEADER_COUNT_FONT_PX
  );
}

/** Cluster header placements: the anchor, then steps up/down the hull and a
 *  slide sideways — the header's centre never leaves its own hull, so it
 *  still reads as that cluster's name. Nearest placements first. */
function headerCandidates(w: number, h: number, gap: number, radius: number): LabelOffset[] {
  const step = h + gap;
  const slide = Math.min(w / 3, radius);
  const dys: number[] = [0];
  for (let k = 1; k * step <= radius; k++) dys.push(-k * step, k * step);
  const out: LabelOffset[] = [];
  for (const dy of dys) {
    for (const dx of [0, -slide, slide]) {
      if (Math.hypot(dx, dy) > radius && !(dx === 0 && dy === 0)) continue;
      out.push(dx === 0 && dy === 0 ? DEFAULT_OFFSET : { dx, dy, key: 'nudge' });
    }
  }
  return out;
}

export function buildLabelBoxes(input: LabelLayoutInput): LabelBox[] {
  const s = invariantScale(input.zoom);
  const gap = LABEL_GAP_PX * s;
  const boxes: LabelBox[] = [];

  if (input.lane) {
    const { lane } = input;
    const ls = HEADER_LETTER_SPACING_EM * HEADER_FONT_PX;
    const text = lane.text.toUpperCase();
    const w = (input.measure(text, HEADER_FONT_PX, 600) + text.length * ls) * s;
    const h = (HEADER_FONT_PX * HEADER_LINE + LANE_HEADER_RULE_PX) * s;
    boxes.push({
      id: 'lane',
      x: lane.x,
      y: lane.y - LANE_HEADER_GAP_PX * s - h,
      w,
      h,
      priority: PRIORITY_LANE,
      candidates: [DEFAULT_OFFSET],
    });
  }

  for (const c of input.headers) {
    const w = headerTextWidth(c.label, c.count, input.measure) * s;
    const h = HEADER_FONT_PX * HEADER_LINE * s;
    boxes.push({
      id: `cluster:${c.id}`,
      x: c.x - w / 2,
      y: c.y,
      w,
      h,
      priority: PRIORITY_HEADER + c.count,
      candidates: headerCandidates(w, h, gap, c.radius),
      weight: WEIGHT_HEADER,
    });
  }

  for (const n of input.nodes) {
    const size = nodeMarkSize(n.memberCount, n.relevance);
    if (n.stack) {
      // A stack mark (and its gold ring) is a soft OBSTACLE: labels step off
      // the user's own dependencies whenever a free placement exists.
      const ring = STACK_RING_PX * s;
      boxes.push({
        id: `${MARK_PREFIX}${n.id}`,
        x: n.x - ring,
        y: n.y - ring,
        w: size + 2 * ring,
        h: size + 2 * ring,
        priority: 0,
        candidates: [DEFAULT_OFFSET],
        weight: WEIGHT_STACK_MARK,
        obstacle: true,
      });
    }
    if (!n.stack && !input.includeNonStack) continue;
    const text = nodeLabelText(n.title);
    const w = Math.min(input.measure(text, NODE_LABEL_FONT_PX, 500), NODE_LABEL_MAX_W) * s;
    const h = NODE_LABEL_FONT_PX * NODE_LABEL_LINE * s;
    boxes.push({
      id: `node:${n.id}`,
      x: n.x + size / 2 - w / 2,
      y: n.y + size + NODE_LABEL_GAP,
      w,
      h,
      priority: (n.stack ? PRIORITY_STACK : 0) + n.relevance * 100,
      weight: n.stack ? WEIGHT_STACK : 1,
      // Below (the default), above, then beside the mark — the renderer maps
      // each key to a placement (content-graph.css, [data-cg-place]).
      candidates: [
        { dx: 0, dy: 0, key: 'below' },
        { dx: 0, dy: -(size + 2 * NODE_LABEL_GAP + h), key: 'above' },
        { dx: size / 2 + w / 2 + NODE_LABEL_GAP, dy: -(size / 2 + NODE_LABEL_GAP + h / 2), key: 'right' },
        { dx: -(size / 2 + w / 2 + NODE_LABEL_GAP), dy: -(size / 2 + NODE_LABEL_GAP + h / 2), key: 'left' },
      ],
    });
  }
  return boxes;
}

/** Everything the renderer needs: which labels show, and where. */
export function layoutGraphLabels(input: LabelLayoutInput): Map<string, LabelPlacement> {
  return resolveLabelCollisions(buildLabelBoxes(input), LABEL_GAP_PX * invariantScale(input.zoom));
}

/** Fallback text measure when no canvas is available (tests, SSR): Inter's
 *  mean advance is ~0.56em for lowercase, ~0.66em for capitals. */
export const estimateTextWidth: MeasureText = (text, fontPx) => {
  let em = 0;
  for (const ch of text) em += ch >= 'A' && ch <= 'Z' ? 0.66 : ch === ' ' ? 0.28 : 0.56;
  return em * fontPx;
};
