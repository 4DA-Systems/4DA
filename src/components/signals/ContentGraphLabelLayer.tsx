// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Runs the label-collision resolver against the live viewport and writes the
// result onto the label elements. Renders nothing; like ZoomCssVar, every
// update bypasses React — no node re-renders per zoom step.
import { useCallback, useEffect, useRef } from 'react';
import { useStore, type Node } from '@xyflow/react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

import {
  LABEL_GAP_PX,
  buildLabelBoxes,
  estimateTextWidth,
  invariantScale,
  resolveLabelCollisions,
  type HeaderLabelInput,
  type LaneLabelInput,
  type LabelPlacement,
  type MeasureText,
  type NodeLabelInput,
} from './content-graph-label-layout';
import { LABEL_DETAIL_ZOOM } from './ContentGraphFlowHelpers';
import { verifyRenderedLabels } from './content-graph-label-verify';

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
      const d = n.data as { title: string; member_count?: number; relevance_score: number; affects_you: boolean; category?: string };
      nodes.push({
        id: n.id,
        x: n.position.x,
        y: n.position.y,
        title: d.title,
        memberCount: d.member_count ?? 1,
        relevance: d.relevance_score,
        stack: d.affects_you,
        security: d.category === 'security',
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

/** One model pass: build the boxes at this zoom, resolve, write placements.
 *  Returns each label's priority for the rendered-rect check that follows. */
export function runLabelPass(host: ParentNode, zoom: number, nodes: Node[], t: TFunction): Map<string, number> {
  const boxes = buildLabelBoxes({
    zoom,
    includeNonStack: zoom >= LABEL_DETAIL_ZOOM,
    measure: measureText,
    ...collectInputs(nodes, t),
  });
  applyLabelPlacements(host, resolveLabelCollisions(boxes, LABEL_GAP_PX * invariantScale(zoom)));
  return new Map(
    boxes.filter((b) => !b.obstacle).map((b) => [b.id, b.id === 'lane' ? Infinity : b.priority]),
  );
}

/** Quiet period after the last pass before checking the rendered rects — a
 *  zoom gesture fires a pass per frame; the DOM read waits for it to settle. */
const VERIFY_DELAY_MS = 120;

export function LabelCollisionLayer() {
  const { t } = useTranslation();
  const zoom = useStore((s) => s.transform[2]);
  const nodes = useStore((s) => s.nodes);
  const ref = useRef<HTMLDivElement | null>(null);
  const latest = useRef<{ zoom: number; nodes: Node[]; t: TFunction } | null>(null);
  const frame = useRef<number | null>(null);
  const verifyTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Zoom / drag / data changes coalesce to one model pass per animation frame
  // (~0.3 ms far, ~2 ms near for 150 nodes — measured); a trailing check of
  // the rendered rects then suppresses anything the model got wrong. Panning
  // never triggers a pass: it moves every label equally.
  const schedule = useCallback(() => {
    if (frame.current !== null) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      const host = ref.current?.closest('.react-flow');
      const cur = latest.current;
      if (!host || !cur) return;
      const priorities = runLabelPass(host, cur.zoom, cur.nodes, cur.t);
      if (verifyTimer.current !== null) clearTimeout(verifyTimer.current);
      verifyTimer.current = setTimeout(() => {
        verifyTimer.current = null;
        verifyRenderedLabels(host, priorities);
      }, VERIFY_DELAY_MS);
    });
  }, []);

  useEffect(() => {
    latest.current = { zoom, nodes, t };
    schedule();
  }, [zoom, nodes, t, schedule]);

  // Text widths are measured in the label font: when webfonts finish loading
  // every cached width is stale — re-measure and re-place.
  useEffect(() => {
    if (!('fonts' in document)) return;
    let alive = true;
    void document.fonts.ready.then(() => {
      if (!alive) return;
      widthCache.clear();
      schedule();
    });
    return () => {
      alive = false;
    };
  }, [schedule]);

  // The cleanup MUST null the ids, not just cancel them. StrictMode (and any
  // offscreen hide/show) runs this cleanup and then re-runs the effects with
  // the SAME refs: a cancelled-but-kept frame id made every later pass return
  // early, so the resolver never ran at all (live 2026-10-04: 0 of 174 labels
  // ever placed, the frame ref stuck on a dead id).
  useEffect(
    () => () => {
      if (frame.current !== null) cancelAnimationFrame(frame.current);
      frame.current = null;
      if (verifyTimer.current !== null) clearTimeout(verifyTimer.current);
      verifyTimer.current = null;
    },
    [],
  );

  return <div ref={ref} style={{ display: 'none' }} />;
}
