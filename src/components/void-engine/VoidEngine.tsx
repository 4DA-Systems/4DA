import { useVoidSignals } from '../../hooks/use-void-signals';
import { VoidHeartbeat } from './VoidHeartbeat';
import { PentachoronMark } from './PentachoronMark';

export type VoidEngineVariant = 'ambient' | 'pentachoron';

interface VoidEngineProps {
  size?: number;
  variant?: VoidEngineVariant;
}

/**
 * Void Engine - renders the signal-aware heartbeat glow.
 * Production component: WebGPU/WebGL2 animation driven by real backend events.
 *
 * Variants:
 * - 'ambient' (default): game-ambient-intelligence — organic pulse shader
 * - 'pentachoron': game-pentachoron — 4D wireframe rotating with golden-ratio speeds
 */
export function VoidEngine({ size = 200, variant = 'ambient' }: VoidEngineProps) {
  const signal = useVoidSignals();

  if (variant === 'pentachoron') {
    return <PentachoronMark signal={signal} size={size} />;
  }

  return (
    <VoidHeartbeat signal={signal} size={size} />
  );
}
