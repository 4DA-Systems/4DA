import { useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

interface VoidCoreProps {
  position: [number, number, number];
}

/**
 * Central glow representing the user's context centroid.
 * A warm pulsing sphere at the origin of the universe.
 */
export function VoidCore({ position }: VoidCoreProps) {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);

  useFrame(({ clock }) => {
    const t = clock.getElapsedTime();
    const breathe = 1.0 + Math.sin(t * 0.8) * 0.15;

    if (meshRef.current) {
      meshRef.current.scale.setScalar(breathe * 0.15);
    }
    if (glowRef.current) {
      glowRef.current.scale.setScalar(breathe * 0.4);
      const mat = glowRef.current.material as THREE.MeshBasicMaterial;
      mat.opacity = 0.12 + Math.sin(t * 0.5) * 0.04;
    }
  });

  return (
    <group position={position}>
      {/* Inner bright core */}
      <mesh ref={meshRef}>
        <sphereGeometry args={[1, 16, 12]} />
        <meshBasicMaterial color="#D4AF37" toneMapped={false} />
      </mesh>

      {/* Outer glow */}
      <mesh ref={glowRef}>
        <sphereGeometry args={[1, 16, 12]} />
        <meshBasicMaterial
          color="#D4AF37"
          transparent
          opacity={0.12}
          toneMapped={false}
          side={THREE.BackSide}
        />
      </mesh>
    </group>
  );
}
