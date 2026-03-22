import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { registerGameComponent, type GameComponentTag } from '../../lib/game-components';

interface GeometryEntry {
  tag: GameComponentTag;
  name: string;
  vertices: number;
  edges: number;
  dimension: string;
  role: string;
  description: string;
}

const GEOMETRIES: GeometryEntry[] = [
  {
    tag: 'game-tetrahedron',
    name: 'Tetrahedron',
    vertices: 4, edges: 6, dimension: '3D',
    role: 'Foundation',
    description: 'The simplest solid. 4 vertices, fully connected. Every pillar supports every other — privacy, local-first, BYOK, zero-config. Self-dual: the product mirrors the user.',
  },
  {
    tag: 'game-pentachoron',
    name: 'Pentachoron',
    vertices: 5, edges: 10, dimension: '4D',
    role: 'Identity',
    description: 'The 4D simplex. 5 tetrahedral cells interlocking. The 5th vertex is you — completing the structure. Rotating in 4D at golden-ratio speeds, projecting intelligence you can feel but never fully see.',
  },
  {
    tag: 'game-icosahedron',
    name: 'Icosahedron',
    vertices: 12, edges: 30, dimension: '3D',
    role: 'Network',
    description: '12 sovereign nodes, each connected to exactly 5 peers. Any message reaches any node in 3 hops. Built from 5 interlocking tetrahedra — the same building blocks as the pentachoron.',
  },
  {
    tag: 'game-simplex-unfold',
    name: 'Simplex Unfold',
    vertices: 5, edges: 10, dimension: '0D→4D',
    role: 'Emergence',
    description: 'Point becomes line becomes triangle becomes tetrahedron becomes pentachoron. Each dimension adds one vertex fully connected to all before it. The minimum structure that encloses maximum volume.',
  },
];

/** Mount a custom element by tag name into a container div */
function GameElementCell({ tag, size }: { tag: string; size: number }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const container = ref.current;
    if (!container || container.children.length > 0) return;
    const el = document.createElement(tag);
    el.style.width = `${size}px`;
    el.style.height = `${size}px`;
    el.style.display = 'block';
    container.appendChild(el);
    return () => {
      if (container.contains(el)) container.removeChild(el);
    };
  }, [tag, size]);

  return <div ref={ref} style={{ width: size, height: size }} />;
}

export function GeometryShowcase() {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState<string | null>(null);

  useEffect(() => {
    GEOMETRIES.forEach(g => registerGameComponent(g.tag));
  }, []);

  return (
    <div className="space-y-4">
      <h4 className="text-sm font-medium text-[#D4AF37] tracking-wide uppercase">
        {t('about.geometryTitle', { defaultValue: 'Platonic Architecture' })}
      </h4>
      <p className="text-xs text-text-secondary leading-relaxed">
        {t('about.geometrySubtitle', { defaultValue: "4DA's identity, architecture, and network topology are aligned with Platonic solids — the most efficient structures mathematics can produce." })}
      </p>

      <div className={`grid ${expanded ? 'grid-cols-1' : 'grid-cols-2'} gap-3`}>
        {GEOMETRIES.map(geo => {
          const isExpanded = expanded === geo.tag;
          const cellSize = isExpanded ? 120 : 64;
          return (
            <button
              key={geo.tag}
              onClick={() => setExpanded(isExpanded ? null : geo.tag)}
              className={`bg-bg-tertiary/40 border rounded-xl p-3 text-left transition-all hover:bg-bg-tertiary/60 ${
                isExpanded ? 'border-[#D4AF37]/40' : 'border-border/50'
              }`}
            >
              <div className="flex items-start gap-3">
                <div
                  className="rounded-lg overflow-hidden border border-border/20 flex-shrink-0"
                  role="img"
                  aria-label={geo.name}
                >
                  <GameElementCell tag={geo.tag} size={cellSize} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-baseline gap-2">
                    <span className="text-xs font-medium text-white">{geo.name}</span>
                    <span className="text-[9px] text-[#D4AF37] font-medium">{geo.role}</span>
                  </div>
                  <span className="text-[10px] text-text-muted block">
                    {geo.dimension} · {geo.vertices}v · {geo.edges}e
                  </span>
                  {isExpanded && (
                    <p className="text-xs text-text-secondary leading-relaxed mt-2">
                      {geo.description}
                    </p>
                  )}
                </div>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
