import { Html } from '@react-three/drei';
import * as THREE from 'three';
import type { VoidInterestNode } from '../../types';

// Distinct colors for interest nodes
const INTEREST_COLORS = [
  '#4fc3f7', '#81c784', '#ffb74d', '#e57373',
  '#ba68c8', '#4dd0e1', '#aed581', '#ff8a65',
  '#f06292', '#7986cb',
];

interface VoidInterestOrbitalsProps {
  interests: VoidInterestNode[];
}

/**
 * Interest orbital nodes - labeled spheres representing topic clusters.
 * Positioned from their embedding projection, colored distinctly.
 */
export function VoidInterestOrbitals({ interests }: VoidInterestOrbitalsProps) {
  if (interests.length === 0) return null;

  return (
    <group>
      {interests.map((interest, i) => (
        <group
          key={interest.name}
          position={interest.position as unknown as THREE.Vector3Tuple}
        >
          {/* Node sphere */}
          <mesh>
            <sphereGeometry args={[0.08 + interest.weight * 0.04, 12, 8]} />
            <meshBasicMaterial
              color={INTEREST_COLORS[i % INTEREST_COLORS.length]}
              transparent
              opacity={0.7}
              toneMapped={false}
            />
          </mesh>

          {/* Label */}
          <Html
            center
            distanceFactor={8}
            style={{
              color: '#fff',
              fontSize: '11px',
              fontFamily: 'Inter, sans-serif',
              fontWeight: 500,
              background: 'rgba(10,10,10,0.8)',
              padding: '2px 6px',
              borderRadius: '3px',
              whiteSpace: 'nowrap',
              pointerEvents: 'none',
              userSelect: 'none',
            }}
          >
            {interest.name}
          </Html>
        </group>
      ))}
    </group>
  );
}
