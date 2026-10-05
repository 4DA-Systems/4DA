// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Category marks for the theme map, its legend and the detail panel.
//
// Shape carries the category; colour is reserved for what needs action. 130
// of 150 live map items were "discussion" (2026-10-05), so a per-category hue
// painted almost everything one blue and the colour channel said nothing.
// Now: security is red, the user's stack is gold, everything else is the
// neutral text tone — and the silhouette keeps categories distinguishable
// without relying on hue (colourblind- and grayscale-safe).

export const GRAPH_CATEGORIES = ['security', 'release', 'discussion', 'research'] as const;

interface Shape {
  borderRadius: string;
  rotate: boolean;
  donut: boolean;
}

const SHAPES: Record<string, Shape> = {
  discussion: { borderRadius: '50%', rotate: false, donut: false },
  release: { borderRadius: '2px', rotate: false, donut: false },
  security: { borderRadius: '1px', rotate: true, donut: false },
  research: { borderRadius: '50%', rotate: false, donut: true },
};

/** The mark's colour: red for security, gold inside the stack column,
 *  neutral otherwise. Tokens only, so both themes work. */
export function markColor(category: string, stack: boolean): string {
  if (category === 'security') return 'var(--color-error)';
  if (stack) return 'var(--color-accent-gold)';
  return 'var(--color-text-muted)';
}

interface MarkProps {
  category: string;
  stack?: boolean;
  size?: number;
  /** Surface behind the mark — fills the research donut's hole. */
  surface?: string;
}

export function CategoryMark({ category, stack = false, size = 8, surface = 'var(--color-bg-secondary)' }: MarkProps) {
  const shape = SHAPES[category] ?? SHAPES.discussion!;
  // A rotated square's diagonal is √2 wider; shrink it to the same footprint.
  const side = shape.rotate ? Math.round(size * 0.78) : size;
  return (
    <span
      aria-hidden="true"
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        width: size,
        height: size,
        flexShrink: 0,
      }}
    >
      <span
        style={{
          position: 'relative',
          display: 'inline-block',
          width: side,
          height: side,
          borderRadius: shape.borderRadius,
          transform: shape.rotate ? 'rotate(45deg)' : undefined,
          backgroundColor: markColor(category, stack),
        }}
      >
        {shape.donut && (
          <span
            style={{
              position: 'absolute',
              inset: '28%',
              borderRadius: '50%',
              backgroundColor: surface,
            }}
          />
        )}
      </span>
    </span>
  );
}
