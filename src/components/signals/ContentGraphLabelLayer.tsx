// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Runs the label-collision resolver against the live viewport and writes the
// result onto the label elements. Renders nothing; like ZoomCssVar, every
// update bypasses React — no node re-renders per zoom step.
import { useEffect, useRef } from 'react';
import { useStore, type Node } from '@xyflow/react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

import {
  estimateTextWidth,
  layoutGraphLabels,
  type HeaderLabelInput,
  type LaneLabelInput,
  type LabelPlacement,
  type MeasureText,
  type NodeLabelInput,
} from './content-graph-label-layout';
import { LABEL_DETAIL_ZOOM } from './ContentGraphFlowHelpers';

let measureCtx: CanvasRenderingContext2D | null | undefined;
const widthCache = new Map<string, number>();

/** Canvas text measure in the label's real font; the estimate stands in where
 *  no canvas exists (jsdom returns a no-op context measuring 0). */
const measureText: MeasureText = (text, fontPx, weight) => {
  const key = `${weight}|${fontPx}|${text}`;
  const cached = widthCache.get(key);
  if (cached !== undefined) return cached;
  if (measureCtx === undefined) {
    try {
      measureCtx = document.createElement('canvas').getContext('2d');
    } catch {
      measureCtx = null;
    }
  }
  let w = 0;
  if (measureCtx) {
    measureCtx.font = `${weight} ${fontPx}px Inter, sans-serif`;
    w = measureCtx.measureText(text).width;
  }
  if (!(w > 0)) w = estimateTextWidth(text, fontPx, weight);
  widthCache.set(key, w);
  return w;
};

interface LayoutInputs {
  nodes: NodeLabelInput[];
  headers: HeaderLabelInput[];
  lane: LaneLabelInput | null;
}

function collectInputs(flowNodes: Node[], t: TFunction): LayoutInputs {
  const nodes: NodeLabelInput[] = [];
  const headers: HeaderLabelInput[] = [];
  let lane: LaneLabelInput | null = null;
  for (const n of flowNodes) {
    if (n.type === 'contentNode') {
      const d = n.data as { title: string; member_count?: number; relevance_score: number; affects_you: boolean };
      nodes.push({
        id: n.id,
        x: n.position.x,
        y: n.position.y,
        title: d.title,
        memberCount: d.member_count ?? 1,
        relevance: d.relevance_score,
        stack: d.affects_you,
      });
    } else if (n.type === 'clusterLabel') {
      const d = n.data as { label: string; count: number; radius?: number };
      headers.push({ id: n.id, x: n.position.x, y: n.position.y, label: d.label, count: d.count, radius: d.radius ?? 0 });
    } else if (n.type === 'laneLabel') {
      const d = n.data as { count: number };
      lane = { x: n.position.x, y: n.position.y, text: t('signals.graphLaneLabel', { count: d.count }) };
    }
  }
  return { nodes, headers, lane };
}

function setAttr(el: HTMLElement, name: string, value: string) {
  if (el.getAttribute(name) !== value) el.setAttribute(name, value);
}

function setVar(el: HTMLElement, name: string, value: string) {
  if (el.style.getPropertyValue(name) !== value) el.style.setProperty(name, value);
}

/** Write placements onto the label elements. A label the resolver did not
 *  consider (a non-stack label at far zoom — content-graph.css hides it by
 *  level of detail) is left unsuppressed at its default placement. */
export function applyLabelPlacements(host: ParentNode, placements: Map<string, LabelPlacement>) {
  host.querySelectorAll<HTMLElement>('[data-cg-label-id]').forEach((el) => {
    const id = el.getAttribute('data-cg-label-id') ?? '';
    const p = placements.get(id);
    setAttr(el, 'data-cg-suppressed', p && !p.visible ? 'true' : 'false');
    if (id.startsWith('node:')) {
      setAttr(el, 'data-cg-place', p?.visible ? p.offset.key : 'below');
    } else {
      setVar(el, '--cg-dx', `${p?.visible ? p.offset.dx : 0}px`);
      setVar(el, '--cg-dy', `${p?.visible ? p.offset.dy : 0}px`);
    }
  });
}

export function LabelCollisionLayer() {
  const { t } = useTranslation();
  const zoom = useStore((s) => s.transform[2]);
  const nodes = useStore((s) => s.nodes);
  const ref = useRef<HTMLDivElement | null>(null);
  const latest = useRef<{ zoom: number; nodes: Node[]; t: TFunction } | null>(null);
  const frame = useRef<number | null>(null);

  // Panning leaves zoom untouched and translates every label equally, so it
  // never triggers a pass. Zoom / drag / data changes coalesce to one pass
  // per animation frame (~0.3 ms far, ~2 ms near for 150 nodes — measured).
  useEffect(() => {
    latest.current = { zoom, nodes, t };
    if (frame.current !== null) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      const host = ref.current?.closest('.react-flow');
      const cur = latest.current;
      if (!host || !cur) return;
      const inputs = collectInputs(cur.nodes, cur.t);
      const placements = layoutGraphLabels({
        zoom: cur.zoom,
        includeNonStack: cur.zoom >= LABEL_DETAIL_ZOOM,
        measure: measureText,
        ...inputs,
      });
      applyLabelPlacements(host, placements);
    });
  }, [zoom, nodes, t]);

  useEffect(
    () => () => {
      if (frame.current !== null) cancelAnimationFrame(frame.current);
    },
    [],
  );

  return <div ref={ref} style={{ display: 'none' }} />;
}
