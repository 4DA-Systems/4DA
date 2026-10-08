// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { render } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';

vi.mock('@xyflow/react', () => ({
  Handle: () => null,
  Position: { Top: 'top', Bottom: 'bottom' },
  useStore: () => undefined,
  useStoreApi: () => ({ getState: () => ({}) }),
}));
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

import ContentGraphNode from './ContentGraphNode';
import { applyLabelPlacements } from './ContentGraphLabelLayer';
import {
  buildLabelBoxes,
  estimateTextWidth,
  invariantScale,
  layoutGraphLabels,
  nodeMarkSize,
  type LabelLayoutInput,
  type NodeLabelInput,
} from './content-graph-label-layout';
import { fitNodeLabelCandidates } from './content-graph-label-fit';

// The measured case (1200x800, PR #871 merged): canvas x 24..1168 screen,
// zoom 0.3447, viewport translate(-214.2, -81.17). In flow units:
const zoom = 0.344738;
const tx = -214.2;
const ty = -81.17;
const canvas = { x: (0 - tx) / zoom, y: (0 - ty) / zoom, w: 1144 / zoom, h: 519 / zoom };
const bounds = { x: canvas.x + 4 / zoom, y: canvas.y + 4 / zoom, w: canvas.w - 8 / zoom, h: canvas.h - 8 / zoom };

const node = (id: string, sx: number, sy: number, title: string, rel = 0.9): NodeLabelInput => ({
  id,
  // Window coords as measured (canvas at window x 24, y 239) -> flow units.
  x: (sx - 24 - tx) / zoom,
  y: (sy - 239 - ty) / zoom,
  title,
  memberCount: 1,
  relevance: rel,
  stack: true,
  security: false,
});

const input = (nodes: NodeLabelInput[]): LabelLayoutInput => ({
  zoom,
  includeNonStack: true,
  nodes,
  headers: [],
  lane: null,
  measure: estimateTextWidth,
  bounds,
});

/** The painted rect of each visible node label (model box + chosen offset). */
function painted(nodes: NodeLabelInput[]) {
  const boxes = buildLabelBoxes(input(nodes)).filter((b) => !b.obstacle);
  const placements = layoutGraphLabels(input(nodes));
  return boxes.map((b) => {
    const p = placements.get(b.id)!;
    return { id: b.id, visible: p.visible, key: p.offset.key, x: b.x + p.offset.dx, y: b.y + p.offset.dy, w: b.w, h: b.h };
  });
}

const insideCanvas = (r: { x: number; y: number; w: number; h: number }) =>
  r.x >= canvas.x - 1e-6 &&
  r.y >= canvas.y - 1e-6 &&
  r.x + r.w <= canvas.x + canvas.w + 1e-6 &&
  r.y + r.h <= canvas.y + canvas.h + 1e-6;

describe('node labels near the canvas edge render fully inside it', () => {
  it('every visible label is inside the canvas: flipped, slid, or suppressed', () => {
    const nodes = [
      node('uuid', 40, 400, 'uuid v1.27.0'), // mark on-canvas at the left edge
      node('singleins', 27, 330, 'tauri-plugin-single-instance v2'),
      node('notif', 3, 293, 'tauri-plugin-notification v2.3'), // mark straddles the edge
      node('rmcp', -7, 346, 'rmcp v3.5.1'), // mark centre off-canvas
      node('openai', -5, 306, 'openai v7.30.0'),
      node('right', 1150, 500, 'A Function-level Dataset of Vulns'),
    ];
    const out = painted(nodes);
    for (const r of out) if (r.visible) expect(insideCanvas(r), r.id).toBe(true);
    const byId = new Map(out.map((r) => [r.id, r]));
    expect(byId.get('node:uuid')!.visible).toBe(true);
    expect(byId.get('node:singleins')!.visible).toBe(true);
    expect(byId.get('node:notif')!.visible).toBe(true);
    expect(byId.get('node:right')!.visible).toBe(true);
    // Marks whose centre is off-canvas get no half-printed label.
    expect(byId.get('node:rmcp')!.visible).toBe(false);
    expect(byId.get('node:openai')!.visible).toBe(false);
  });

  it('slides a label along its side when a flip alone does not fit', () => {
    const s = invariantScale(zoom);
    const size = nodeMarkSize(1, 0.9);
    const w = 120 * s;
    const h = 13 * s;
    // Mark straddling the left edge: below is off-canvas, right fits after a
    // vertical slide only when near the top; force the below-slide case.
    const mark = { x: bounds.x - size / 2, y: bounds.y + 200, w: size, h: size };
    const box = { x: mark.x + size / 2 - w / 2, y: mark.y + size + 3, w, h };
    const cands = fitNodeLabelCandidates(box, mark, [{ dx: 0, dy: 0, key: 'below' }], bounds, s);
    expect(cands).toHaveLength(1);
    expect(cands[0]!.sx).toBeCloseTo(bounds.x - box.x);
    expect(box.x + cands[0]!.dx).toBeCloseTo(bounds.x);
  });

  it('without bounds every candidate is offered unchanged', () => {
    const c = [{ dx: 0, dy: 0, key: 'below' }];
    expect(fitNodeLabelCandidates({ x: -500, y: 0, w: 10, h: 10 }, { x: -500, y: 0, w: 5, h: 5 }, c, undefined, 1)).toBe(c);
  });
});

describe('ContentGraphNode label paints the box the resolver placed', () => {
  const data = {
    id: 1, title: 'crates.io: uuid v1.27.0', category: 'release', member_count: 1, relevance_score: 0.9,
    affects_you: true, signal_priority: null, source_type: 'crates_io',
  };

  it('is shrink-to-fit (max-width), not a fixed 150px box', () => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const { container } = render(<ContentGraphNode {...({ id: 'n1', data, selected: false } as any)} />);
    const el = container.querySelector<HTMLElement>('.cg-node-label')!;
    expect(el.textContent).toBe('uuid v1.27.0');
    expect(el.style.width).toBe('');
    expect(el.style.maxWidth).toContain('150px');
  });

  it('carries the slide onto the label as --cg-dx / --cg-dy', () => {
    const host = document.createElement('div');
    host.innerHTML = '<span data-cg-label-id="node:n1"></span><span data-cg-label-id="node:n2"></span>';
    applyLabelPlacements(
      host,
      new Map([
        ['node:n1', { visible: true, offset: { dx: 40, dy: 0, key: 'below', sx: 40, sy: 0 } }],
        ['node:n2', { visible: false, offset: { dx: 0, dy: 0, key: 'default' } }],
      ]),
    );
    const [a, b] = [...host.querySelectorAll<HTMLElement>('span')];
    expect(a!.getAttribute('data-cg-place')).toBe('below');
    expect(a!.style.getPropertyValue('--cg-dx')).toBe('40px');
    expect(b!.getAttribute('data-cg-suppressed')).toBe('true');
    expect(b!.style.getPropertyValue('--cg-dx')).toBe('0px');
  });
});
