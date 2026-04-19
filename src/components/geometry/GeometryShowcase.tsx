// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { type ReactNode, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PlatonicSVG } from './PlatonicSVG';
import { PentachoronSVG } from './PentachoronSVG';
import { CompoundFiveSVG } from './CompoundFiveSVG';
import { SimplexUnfoldSVG } from './SimplexUnfoldSVG';
import { TETRAHEDRON, ICOSAHEDRON, DODECAHEDRON } from './geometries';

interface GeometryEntry {
  id: string;
  name: string;
  vertices: number;
  edges: number;
  dimension: string;
  role: string;
  description: string;
  verify?: string;
  render: (size: number) => ReactNode;
}

const FOUNDATIONS: GeometryEntry[] = [
  {
    id: 'tetrahedron',
    name: 'Tetrahedron',
    vertices: 4, edges: 6, dimension: '3D',
    role: 'Foundation',
    description: 'Four non-negotiable invariants: privacy (INV-004), BYOK (INV-031), local-first (INV-032), zero-config (INV-002). Each enforces every other \u2014 BYOK requires privacy, privacy requires local-first, local-first enables zero-config. Remove any vertex and the solid collapses.',
    verify: '.ai/INVARIANTS.md',
    render: (size: number) => (
      <PlatonicSVG
        vertices={TETRAHEDRON.vertices}
        edges={TETRAHEDRON.edges}
        faces={TETRAHEDRON.faces}
        size={size}
        rotationSpeed={0.012}
      />
    ),
  },
  {
    id: 'pentachoron',
    name: 'Pentachoron',
    vertices: 5, edges: 10, dimension: '4D',
    role: 'Identity',
    description: '4 Dimensional Autonomy \u2014 the name is literal. Four architectural invariants plus your context. ACE scans your projects, learns your stack, maps your interests. The system has four pillars; your context makes five. Rotation speeds are golden-ratio-derived for non-repeating motion.',
    verify: 'src-tauri/src/ace/',
    render: (size: number) => <PentachoronSVG size={size} />,
  },
  {
    id: 'simplex-unfold',
    name: 'Simplex Unfold',
    vertices: 5, edges: 10, dimension: '0D\u20264D',
    role: 'Emergence',
    description: 'The simplex progression: each dimension adds one vertex fully connected to all before it. Point, line, triangle, tetrahedron, pentachoron \u2014 minimum structure, maximum volume at every scale. Five phases animate the journey from 0D to 4D.',
    render: (size: number) => <SimplexUnfoldSVG size={size} />,
  },
];

const FAMILY: GeometryEntry[] = [
  {
    id: 'icosahedron',
    name: 'Icosahedron',
    vertices: 12, edges: 30, dimension: '3D',
    role: 'Network',
    description: '12 vertices, each connected to exactly 5 neighbours. Any node reaches any other in 3 hops. The most efficient triangulated sphere \u2014 design target for the distributed Team Relay network.',
    verify: 'docs/strategy/TEAM-RELAY-ARCHITECTURE.md',
    render: (size: number) => (
      <PlatonicSVG
        vertices={ICOSAHEDRON.vertices}
        edges={ICOSAHEDRON.edges}
        faces={ICOSAHEDRON.faces}
        size={size}
        rotationSpeed={0.008}
      />
    ),
  },
  {
    id: 'dodecahedron',
    name: 'Dodecahedron',
    vertices: 20, edges: 30, dimension: '3D',
    role: 'Dual',
    description: 'The icosahedron\u2019s mathematical dual \u2014 same 30 edges, pentagons where triangles were. Every Platonic solid has a dual. 4DA\u2019s visual language honours the full family.',
    render: (size: number) => (
      <PlatonicSVG
        vertices={DODECAHEDRON.vertices}
        edges={DODECAHEDRON.edges}
        faces={DODECAHEDRON.faces}
        size={size}
        rotationSpeed={0.007}
      />
    ),
  },
  {
    id: 'compound-five',
    name: 'Compound of Five',
    vertices: 20, edges: 30, dimension: '3D',
    role: 'Bridge',
    description: 'Five interlocking tetrahedra whose vertices coincide with the dodecahedron\u2019s. The proof these solids aren\u2019t arbitrary \u2014 shared vertices, edges, and duality connect the entire Platonic family.',
    render: (size: number) => <CompoundFiveSVG size={size} />,
  },
];

function GeometryCard({ geo, isExpanded, onToggle }: {
  geo: GeometryEntry;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const displaySize = isExpanded ? 240 : 180;
  return (
    <button
      onClick={onToggle}
      className={`bg-bg-tertiary/40 border rounded-xl p-2 text-start transition-all hover:bg-bg-tertiary/60 ${
        isExpanded ? 'border-accent-gold/40 col-span-2' : 'border-border/50'
      }`}
    >
      <div className="flex justify-center">
        <div
          className="rounded-lg overflow-hidden aspect-square w-full flex items-center justify-center"
          style={{ maxWidth: displaySize }}
          role="img"
          aria-label={geo.name}
        >
          {geo.render(displaySize)}
        </div>
      </div>
      <div className="text-center mt-1.5">
        <span className="text-xs font-medium text-white">{geo.name}</span>
        <span className="text-[9px] text-accent-gold font-medium ml-1.5">{geo.role}</span>
        <div className="text-[9px] text-text-muted">
          {geo.dimension}{' \u00B7 '}{geo.vertices}{'v \u00B7 '}{geo.edges}{'e'}
        </div>
      </div>
      {isExpanded && (
        <div className="mt-2 text-start">
          <p className="text-xs text-text-secondary leading-relaxed">
            {geo.description}
          </p>
          {geo.verify != null && geo.verify !== '' && (
            <p className="text-[10px] text-text-muted/70 mt-1.5 font-mono">
              {'verify: '}{geo.verify}
            </p>
          )}
        </div>
      )}
    </button>
  );
}

export function GeometryShowcase() {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState<string | null>(null);
  const [showFamily, setShowFamily] = useState(false);

  const toggle = (id: string) => setExpanded(prev => prev === id ? null : id);

  return (
    <div className="space-y-4">
      <h4 className="text-sm font-medium text-accent-gold tracking-wide uppercase">
        {t('about.geometryTitle')}
      </h4>
      <p className="text-xs text-text-secondary leading-relaxed">
        {t('about.geometrySubtitle')}
      </p>

      <div className="grid grid-cols-2 gap-2">
        {FOUNDATIONS.map(geo => (
          <GeometryCard
            key={geo.id}
            geo={geo}
            isExpanded={expanded === geo.id}
            onToggle={() => toggle(geo.id)}
          />
        ))}
      </div>

      <button
        onClick={() => setShowFamily(prev => !prev)}
        className="flex items-center gap-1.5 text-[10px] text-text-muted hover:text-text-secondary transition-colors uppercase tracking-wider"
      >
        <span className="text-[8px]">{showFamily ? '\u25BC' : '\u25B6'}</span>
        {showFamily ? t('about.hideFamily') : t('about.showFamily')}
        <span className="text-text-muted/50">{'\u00B7 '}{t('about.moreSolids')}</span>
      </button>

      {showFamily && (
        <div className="grid grid-cols-2 gap-2">
          {FAMILY.map(geo => (
            <GeometryCard
              key={geo.id}
              geo={geo}
              isExpanded={expanded === geo.id}
              onToggle={() => toggle(geo.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
