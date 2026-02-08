import { useState, useCallback, lazy, Suspense } from 'react';
import { useVoidSignals } from '../../hooks/use-void-signals';
import { VoidHeartbeat } from './VoidHeartbeat';

// Lazy-load the universe (Three.js bundle) - zero startup cost
const VoidUniverse = lazy(() => import('./VoidUniverse'));

interface VoidEngineProps {
  size?: number;
}

/**
 * Void Engine orchestrator.
 * Renders heartbeat in ambient mode.
 * Click to expand into full 3D universe (lazy-loads Three.js).
 */
export function VoidEngine({ size = 200 }: VoidEngineProps) {
  const signal = useVoidSignals();
  const [expanded, setExpanded] = useState(false);

  const handleExpand = useCallback(() => {
    setExpanded(true);
  }, []);

  const handleCollapse = useCallback(() => {
    setExpanded(false);
  }, []);

  return (
    <>
      {/* Heartbeat - always rendered, clickable to expand */}
      <div
        onClick={handleExpand}
        style={{ cursor: 'pointer' }}
        title="Click to explore your information universe"
      >
        <VoidHeartbeat signal={signal} size={size} />
      </div>

      {/* Universe - fullscreen overlay, lazy-loaded */}
      {expanded && (
        <Suspense
          fallback={
            <div
              style={{
                position: 'fixed',
                inset: 0,
                background: '#0A0A0A',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: '#666',
                fontFamily: 'Inter, sans-serif',
                fontSize: 14,
                zIndex: 1000,
              }}
            >
              Loading universe...
            </div>
          }
        >
          <VoidUniverse onClose={handleCollapse} />
        </Suspense>
      )}
    </>
  );
}
