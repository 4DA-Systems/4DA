import type { VoidParticle } from '../../types';

interface VoidSelectionPanelProps {
  particle: VoidParticle;
  detail: Record<string, unknown> | null;
  neighbors: VoidParticle[];
  onClose: () => void;
  onSelectNeighbor: (particle: VoidParticle) => void;
}

/**
 * 2D overlay panel showing detail for a selected particle.
 * Appears on the right side of the universe view.
 */
export function VoidSelectionPanel({
  particle,
  detail,
  neighbors,
  onClose,
  onSelectNeighbor,
}: VoidSelectionPanelProps) {
  const sourceColors: Record<string, string> = {
    hackernews: '#ff6600',
    arxiv: '#b31b1b',
    reddit: '#ff4500',
    github: '#238636',
    producthunt: '#da552f',
    rss: '#f99b2b',
    twitter: '#1da1f2',
    youtube: '#ff0000',
  };

  const color = sourceColors[particle.source_type] || '#666';

  return (
    <div
      style={{
        position: 'absolute',
        top: 16,
        right: 16,
        width: 320,
        maxHeight: 'calc(100% - 32px)',
        background: 'rgba(20, 20, 20, 0.95)',
        border: '1px solid #2A2A2A',
        borderRadius: 8,
        padding: 16,
        color: '#fff',
        fontFamily: 'Inter, sans-serif',
        fontSize: 13,
        overflow: 'auto',
        zIndex: 10,
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start', marginBottom: 12 }}>
        <div style={{ flex: 1 }}>
          <span
            style={{
              display: 'inline-block',
              padding: '2px 6px',
              borderRadius: 3,
              background: color,
              color: '#fff',
              fontSize: 10,
              fontWeight: 600,
              textTransform: 'uppercase',
              marginBottom: 6,
            }}
          >
            {particle.source_type}
          </span>
          <div style={{ fontWeight: 600, fontSize: 14, lineHeight: 1.3 }}>
            {particle.label}
          </div>
        </div>
        <button
          onClick={onClose}
          style={{
            background: 'none',
            border: 'none',
            color: '#666',
            cursor: 'pointer',
            fontSize: 18,
            padding: '0 0 0 8px',
            lineHeight: 1,
          }}
        >
          x
        </button>
      </div>

      {/* URL */}
      {particle.url && (
        <a
          href={particle.url}
          target="_blank"
          rel="noopener noreferrer"
          style={{ color: '#4fc3f7', fontSize: 12, wordBreak: 'break-all', display: 'block', marginBottom: 8 }}
        >
          {particle.url}
        </a>
      )}

      {/* Detail */}
      {detail && detail.content_preview ? (
        <div style={{ marginBottom: 12 }}>
          <p style={{ color: '#A0A0A0', fontSize: 12, lineHeight: 1.5, margin: '8px 0' }}>
            {String(detail.content_preview)}
          </p>
        </div>
      ) : null}

      {/* Meta */}
      <div style={{ color: '#666', fontSize: 11, marginBottom: 12 }}>
        <span>Layer: {particle.layer}</span>
        {particle.age_hours > 0 && (
          <span style={{ marginLeft: 12 }}>
            Age: {particle.age_hours < 24
              ? `${Math.round(particle.age_hours)}h`
              : `${Math.round(particle.age_hours / 24)}d`}
          </span>
        )}
      </div>

      {/* Neighbors */}
      {neighbors.length > 0 && (
        <div>
          <div style={{ color: '#A0A0A0', fontSize: 11, fontWeight: 600, marginBottom: 6, textTransform: 'uppercase' }}>
            Nearby Items
          </div>
          {neighbors.slice(0, 6).map((n) => (
            <button
              key={`${n.layer}-${n.id}`}
              onClick={() => onSelectNeighbor(n)}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                background: 'rgba(255,255,255,0.03)',
                border: '1px solid #2A2A2A',
                borderRadius: 4,
                padding: '6px 8px',
                marginBottom: 4,
                cursor: 'pointer',
                color: '#ccc',
                fontSize: 12,
              }}
            >
              <span style={{ color: sourceColors[n.source_type] || '#666', marginRight: 6, fontSize: 10 }}>
                {n.source_type}
              </span>
              {n.label.slice(0, 60)}{n.label.length > 60 ? '...' : ''}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
