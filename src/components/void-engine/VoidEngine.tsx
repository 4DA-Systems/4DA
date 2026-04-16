import { useVoidSignals } from '../../hooks/use-void-signals';
import { PentachoronMark } from './PentachoronMark';

interface VoidEngineProps {
  size?: number;
}

/**
 * Void Engine — renders the signal-responsive pentachoron at large sizes.
 *
 * Used in GeometryShowcase and similar contexts where the full 4D wireframe
 * is displayed at 200px+. For app bar / small brand mark usage, use BrandMark instead.
 */
export function VoidEngine({ size = 200 }: VoidEngineProps) {
  const signal = useVoidSignals();
  return <PentachoronMark signal={signal} size={size} />;
}
