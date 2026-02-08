import { useRef, useMemo, useCallback } from 'react';
import { useFrame, ThreeEvent } from '@react-three/fiber';
import * as THREE from 'three';
import type { VoidParticle } from '../../types';

// Source type -> color mapping
const SOURCE_COLORS: Record<string, THREE.Color> = {
  hackernews: new THREE.Color(0xff6600),
  arxiv: new THREE.Color(0xb31b1b),
  reddit: new THREE.Color(0xff4500),
  github: new THREE.Color(0x238636),
  producthunt: new THREE.Color(0xda552f),
  rss: new THREE.Color(0xf99b2b),
  twitter: new THREE.Color(0x1da1f2),
  youtube: new THREE.Color(0xff0000),
};
const CONTEXT_COLOR = new THREE.Color(0x8888ff);
const DEFAULT_COLOR = new THREE.Color(0x666666);
const DIM_FACTOR = 0.15; // Opacity for non-matching particles during search

interface VoidParticlesProps {
  particles: VoidParticle[];
  selectedId: number | null;
  selectedLayer: string | null;
  onSelect: (particle: VoidParticle) => void;
  searchFilter: Set<string> | null; // null = no filter, Set = highlight only matching
}

export function VoidParticles({
  particles,
  selectedId,
  selectedLayer,
  onSelect,
  searchFilter,
}: VoidParticlesProps) {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const dummy = useMemo(() => new THREE.Object3D(), []);
  const colorArray = useMemo(() => new Float32Array(particles.length * 3), [particles.length]);

  // Build instance matrices and colors
  useMemo(() => {
    if (!meshRef.current) return;
    const mesh = meshRef.current;

    particles.forEach((p, i) => {
      dummy.position.set(p.position[0], p.position[1], p.position[2]);

      const isSelected = p.id === selectedId && p.layer === selectedLayer;
      const key = `${p.layer}-${p.id}`;
      const isSearchMatch = !searchFilter || searchFilter.has(key);

      // Size: selected = 3x, search match = 1.2x, normal source = 1x, context = 0.7x
      const baseSize = p.layer === 'context' ? 0.03 : 0.05;
      let scale = baseSize;
      if (isSelected) scale = baseSize * 3;
      else if (searchFilter && isSearchMatch) scale = baseSize * 1.3;
      else if (searchFilter && !isSearchMatch) scale = baseSize * 0.6;
      dummy.scale.setScalar(scale);

      dummy.updateMatrix();
      mesh.setMatrixAt(i, dummy.matrix);

      // Color by source type, dimmed if search-filtered out
      const color =
        p.layer === 'context'
          ? CONTEXT_COLOR
          : SOURCE_COLORS[p.source_type] || DEFAULT_COLOR;

      const dim = searchFilter && !isSearchMatch ? DIM_FACTOR : 1.0;
      colorArray[i * 3] = color.r * dim;
      colorArray[i * 3 + 1] = color.g * dim;
      colorArray[i * 3 + 2] = color.b * dim;
    });

    mesh.instanceMatrix.needsUpdate = true;
    mesh.geometry.setAttribute(
      'color',
      new THREE.InstancedBufferAttribute(colorArray, 3),
    );
  }, [particles, selectedId, selectedLayer, searchFilter, dummy, colorArray]);

  // Gentle rotation for liveliness
  useFrame((_, delta) => {
    if (meshRef.current) {
      meshRef.current.rotation.y += delta * 0.01;
    }
  });

  // Click handler using R3F raycasting
  const handleClick = useCallback(
    (e: ThreeEvent<MouseEvent>) => {
      e.stopPropagation();
      if (e.instanceId !== undefined && e.instanceId < particles.length) {
        onSelect(particles[e.instanceId]);
      }
    },
    [particles, onSelect],
  );

  if (particles.length === 0) return null;

  return (
    <instancedMesh
      ref={meshRef}
      args={[undefined, undefined, particles.length]}
      onClick={handleClick}
    >
      <sphereGeometry args={[1, 8, 6]} />
      <meshBasicMaterial vertexColors toneMapped={false} transparent opacity={0.85} />
    </instancedMesh>
  );
}
