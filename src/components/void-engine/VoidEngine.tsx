import { useVoidSignals } from '../../hooks/use-void-signals';
import { VoidHeartbeat } from './VoidHeartbeat';

interface VoidEngineProps {
  size?: number;
}

/**
 * Void Engine - renders the signal-aware heartbeat glow.
 * Production component: CSS/WebGL2 animation driven by real backend events.
 */
export function VoidEngine({ size = 200 }: VoidEngineProps) {
  const signal = useVoidSignals();

  return (
    <VoidHeartbeat signal={signal} size={size} />
  );
}
