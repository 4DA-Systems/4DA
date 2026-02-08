interface VoidHUDProps {
  totalItems: number;
  particleCount: number;
  interestCount: number;
  clusterCount: number;
  loading: boolean;
  onClose: () => void;
  onRefresh: () => void;
}

/**
 * Heads-up display overlay for the universe view.
 * Shows stats and controls in the corners.
 */
export function VoidHUD({
  totalItems,
  particleCount,
  interestCount,
  clusterCount,
  loading,
  onClose,
  onRefresh,
}: VoidHUDProps) {
  return (
    <>
      {/* Top-left: Stats */}
      <div
        style={{
          position: 'absolute',
          top: 16,
          left: 16,
          color: '#666',
          fontFamily: 'JetBrains Mono, monospace',
          fontSize: 11,
          lineHeight: 1.6,
          pointerEvents: 'none',
          zIndex: 10,
        }}
      >
        <div style={{ color: '#A0A0A0', fontWeight: 600, marginBottom: 4 }}>VOID UNIVERSE</div>
        <div>Particles: {particleCount.toLocaleString()}</div>
        <div>Total Items: {totalItems.toLocaleString()}</div>
        {interestCount > 0 && <div>Interests: {interestCount}</div>}
        {clusterCount > 0 && <div>Clusters: {clusterCount}</div>}
        {loading && <div style={{ color: '#D4AF37' }}>Loading...</div>}
      </div>

      {/* Bottom-left: Keyboard shortcuts */}
      <div
        style={{
          position: 'absolute',
          bottom: 16,
          left: 16,
          color: '#333',
          fontFamily: 'JetBrains Mono, monospace',
          fontSize: 10,
          lineHeight: 2,
          pointerEvents: 'none',
          zIndex: 10,
        }}
      >
        <div><Kbd>Esc</Kbd> Close / Deselect</div>
        <div><Kbd>/</Kbd> Search</div>
        <div><Kbd>H</Kbd> Home view</div>
        <div><Kbd>F</Kbd> Focus selected</div>
        <div><Kbd>R</Kbd> Refresh</div>
        <div><Kbd>1-9</Kbd> Fly to interest</div>
        <div style={{ marginTop: 4, color: '#444' }}>Drag to orbit | Scroll to zoom</div>
      </div>

      {/* Top-right: Close + Refresh buttons */}
      <div
        style={{
          position: 'absolute',
          top: 16,
          right: 16,
          display: 'flex',
          gap: 8,
          zIndex: 10,
        }}
      >
        <button
          onClick={onRefresh}
          title="Refresh universe (R)"
          style={buttonStyle}
        >
          Refresh
        </button>
        <button
          onClick={onClose}
          title="Close universe (Esc)"
          style={buttonStyle}
        >
          Close
        </button>
      </div>
    </>
  );
}

const buttonStyle: React.CSSProperties = {
  background: 'rgba(20,20,20,0.8)',
  border: '1px solid #2A2A2A',
  borderRadius: 6,
  color: '#A0A0A0',
  cursor: 'pointer',
  padding: '6px 10px',
  fontSize: 12,
  fontFamily: 'Inter, sans-serif',
};

/** Inline keyboard shortcut badge */
function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <span
      style={{
        display: 'inline-block',
        background: 'rgba(255,255,255,0.06)',
        border: '1px solid #333',
        borderRadius: 3,
        padding: '0 4px',
        fontSize: 9,
        fontFamily: 'JetBrains Mono, monospace',
        color: '#555',
        marginRight: 4,
        minWidth: 16,
        textAlign: 'center',
      }}
    >
      {children}
    </span>
  );
}
