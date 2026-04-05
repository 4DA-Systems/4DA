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
  verify?: string;
}

// Architecturally grounded — each maps to verifiable system properties
const FOUNDATIONS: GeometryEntry[] = [
  {
    tag: 'game-tetrahedron',
    name: 'Tetrahedron',
    vertices: 4, edges: 6, dimension: '3D',
    role: 'Foundation',
    description: 'Four non-negotiable invariants: privacy (INV-004), BYOK (INV-031), local-first (INV-032), zero-config (INV-002). Each enforces every other — BYOK requires privacy, privacy requires local-first, local-first enables zero-config. Remove any vertex and the solid collapses.',
    verify: '.ai/INVARIANTS.md',
  },
  {
    tag: 'game-pentachoron',
    name: 'Pentachoron',
    vertices: 5, edges: 10, dimension: '4D',
    role: 'Identity',
    description: '4 Dimensional Autonomy — the name is literal. Four architectural invariants plus your context. ACE scans your projects, learns your stack, maps your interests. The system has four pillars; your context makes five. Rotation speeds are golden-ratio-derived (0.618, 0.382) for non-repeating motion.',
    verify: 'src-tauri/src/ace/',
  },
  {
    tag: 'game-simplex-unfold',
    name: 'Simplex Unfold',
    vertices: 5, edges: 10, dimension: '0D\u20264D',
    role: 'Emergence',
    description: 'The 4DA mark. Each dimension adds one vertex fully connected to all before it — minimum structure, maximum volume at every scale. Point, line, triangle, tetrahedron, pentachoron. This is what the product does: simple inputs compound into intelligence.',
  },
];

// Geometric family — visual identity completing the Platonic solid set
const FAMILY: GeometryEntry[] = [
  {
    tag: 'game-icosahedron',
    name: 'Icosahedron',
    vertices: 12, edges: 30, dimension: '3D',
    role: 'Network',
    description: '12 vertices, each connected to exactly 5 neighbours. Any node reaches any other in 3 hops. The most efficient triangulated sphere — design target for the distributed Team Relay network.',
    verify: 'docs/strategy/TEAM-RELAY-ARCHITECTURE.md',
  },
  {
    tag: 'game-dodecahedron',
    name: 'Dodecahedron',
    vertices: 20, edges: 30, dimension: '3D',
    role: 'Dual',
    description: "The icosahedron\u2019s mathematical dual \u2014 same 30 edges, pentagons where triangles were. Every Platonic solid has a dual. 4DA\u2019s visual language honours the full family.",
  },
  {
    tag: 'game-compound-five-tetrahedra',
    name: 'Compound of Five',
    vertices: 20, edges: 30, dimension: '3D',
    role: 'Bridge',
    description: "Five interlocking tetrahedra whose vertices coincide with the dodecahedron\u2019s. The proof these solids aren\u2019t arbitrary \u2014 shared vertices, edges, and duality connect the entire Platonic family.",
  },
];

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

function GeometryCard({ geo, isExpanded, onToggle }: {
  geo: GeometryEntry;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const cellSize = isExpanded ? 120 : 64;
  return (
    <button
      onClick={onToggle}
      className={`bg-bg-tertiary/40 border rounded-xl p-3 text-start transition-all hover:bg-bg-tertiary/60 ${
        isExpanded ? 'border-accent-gold/40' : 'border-border/50'
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
            <span className="text-[9px] text-accent-gold font-medium">{geo.role}</span>
          </div>
          <span className="text-[10px] text-text-muted block">
            {geo.dimension} · {geo.vertices}v · {geo.edges}e
          </span>
          {isExpanded && (
            <>
              <p className="text-xs text-text-secondary leading-relaxed mt-2">
                {geo.description}
              </p>
              {geo.verify && (
                <p className="text-[10px] text-text-muted/70 mt-1.5 font-mono">
                  verify: {geo.verify}
                </p>
              )}
            </>
          )}
        </div>
      </div>
    </button>
  );
}

export function GeometryShowcase() {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState<string | null>(null);
  const [showFamily, setShowFamily] = useState(false);

  useEffect(() => {
    FOUNDATIONS.forEach(g => registerGameComponent(g.tag));
  }, []);

  useEffect(() => {
    if (showFamily) {
      FAMILY.forEach(g => registerGameComponent(g.tag));
    }
  }, [showFamily]);

  const toggle = (tag: string) => setExpanded(prev => prev === tag ? null : tag);

  const handleFamilyToggle = () => {
    if (showFamily && expanded && FAMILY.some(g => g.tag === expanded)) {
      setExpanded(null);
    }
    setShowFamily(prev => !prev);
  };

  const foundationExpanded = expanded !== null && FOUNDATIONS.some(g => g.tag === expanded);
  const familyExpanded = expanded !== null && FAMILY.some(g => g.tag === expanded);

  return (
    <div className="space-y-4">
      <h4 className="text-sm font-medium text-accent-gold tracking-wide uppercase">
        {t('about.geometryTitle', { defaultValue: 'Platonic Architecture' })}
      </h4>
      <p className="text-xs text-text-secondary leading-relaxed">
        {t('about.geometrySubtitle', { defaultValue: "4DA\u2019s core invariants map to Platonic geometry. Each claim below is verifiable against the referenced source files." })}
      </p>

      <div className={`grid ${foundationExpanded ? 'grid-cols-1' : 'grid-cols-2'} gap-3`}>
        {FOUNDATIONS.map(geo => (
          <GeometryCard
            key={geo.tag}
            geo={geo}
            isExpanded={expanded === geo.tag}
            onToggle={() => toggle(geo.tag)}
          />
        ))}
      </div>

      <button
        onClick={handleFamilyToggle}
        className="flex items-center gap-1.5 text-[10px] text-text-muted hover:text-text-secondary transition-colors uppercase tracking-wider"
      >
        <span className="text-[8px]">{showFamily ? '\u25BC' : '\u25B6'}</span>
        {showFamily ? 'Hide geometric family' : 'Show geometric family'}
        <span className="text-text-muted/50">{'\u00B7'} 3 more solids</span>
      </button>

      {showFamily && (
        <div className={`grid ${familyExpanded ? 'grid-cols-1' : 'grid-cols-2'} gap-3`}>
          {FAMILY.map(geo => (
            <GeometryCard
              key={geo.tag}
              geo={geo}
              isExpanded={expanded === geo.tag}
              onToggle={() => toggle(geo.tag)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
