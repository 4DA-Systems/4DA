// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useMemo } from 'react';
import type { ReactNode } from 'react';
import { SPRITE_COLS, SPRITE_ROWS, spriteFrames } from './brand-mark-geometry';
import type { BrandMarkFrame } from './brand-mark-geometry';

interface SheetProps {
  size: number;
  filterId: string;
}

const CELL = 100; // each frame keeps the mark's original 0..100 viewBox

function cellPos(i: number) {
  return { x: (i % SPRITE_COLS) * CELL, y: Math.floor(i / SPRITE_COLS) * CELL };
}

/** Nested <svg> per frame: it clips the frame (and its blur) to its own cell. */
function Cell({ index, children }: { index: number; children: ReactNode }) {
  const { x, y } = cellPos(index);
  return (
    <svg
      x={x}
      y={y}
      width={CELL}
      height={CELL}
      viewBox={`0 0 ${CELL} ${CELL}`}
      overflow="hidden"
      data-frame={index}
    >
      {children}
    </svg>
  );
}

function SharpFrame({ frame }: { frame: BrandMarkFrame }) {
  return (
    <>
      {frame.faces.map((f, i) => (
        <polygon key={`f${i}`} className="bm-face" points={f.points} opacity={f.opacity} />
      ))}
      {frame.edges.map((e, i) => (
        <line
          key={`e${i}`}
          className="bm-edge"
          strokeLinecap="round"
          x1={e.x1}
          y1={e.y1}
          x2={e.x2}
          y2={e.y2}
          strokeWidth={e.strokeWidth}
          opacity={e.opacity}
        />
      ))}
      {frame.verts.map((v, i) => (
        <circle key={`v${i}`} className="bm-vert" cx={v.cx} cy={v.cy} r={v.r} opacity={v.opacity} />
      ))}
    </>
  );
}

/**
 * The whole loop of the mark as one static sprite sheet: a glow sheet (the
 * blurred edges, its own opacity-animated layer) under a sharp sheet (faces,
 * edges, vertices). Rendered once per size bin; colours come from CSS custom
 * properties on the container, so a signal change never re-renders the frames.
 */
export const BrandMarkSprite = memo(function BrandMarkSprite({ size, filterId }: SheetProps) {
  const frames = useMemo(() => spriteFrames(size), [size]);
  const width = SPRITE_COLS * size;
  const height = SPRITE_ROWS * size;
  const viewBox = `0 0 ${SPRITE_COLS * CELL} ${SPRITE_ROWS * CELL}`;

  return (
    <div className="brand-mark-sprite" style={{ width, height }} data-testid="brand-mark-sprite">
      <svg
        className="brand-mark-sheet brand-mark-glow"
        width={width}
        height={height}
        viewBox={viewBox}
        fill="none"
        aria-hidden="true"
      >
        <defs>
          <filter id={filterId} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        {frames.map((frame, i) => (
          <Cell key={i} index={i}>
            <g filter={`url(#${filterId})`}>
              {frame.glow.map((e, j) => (
                <line
                  key={j}
                  className="bm-glow-line"
                  strokeLinecap="round"
                  x1={e.x1}
                  y1={e.y1}
                  x2={e.x2}
                  y2={e.y2}
                  strokeWidth={e.strokeWidth}
                  opacity={e.opacity}
                />
              ))}
            </g>
          </Cell>
        ))}
      </svg>
      <svg
        className="brand-mark-sheet brand-mark-sharp"
        width={width}
        height={height}
        viewBox={viewBox}
        fill="none"
        aria-hidden="true"
      >
        {frames.map((frame, i) => (
          <Cell key={i} index={i}>
            <SharpFrame frame={frame} />
          </Cell>
        ))}
      </svg>
    </div>
  );
});
