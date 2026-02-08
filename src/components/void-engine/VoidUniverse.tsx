import { useEffect, useCallback, useRef, useState } from 'react';
import { Canvas, useThree, useFrame } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import * as THREE from 'three';
import { VoidCore } from './VoidCore';
import { VoidParticles } from './VoidParticles';
import { VoidInterestOrbitals } from './VoidInterestOrbitals';
import { VoidSelectionPanel } from './VoidSelectionPanel';
import { VoidHUD } from './VoidHUD';
import { useVoidUniverse } from '../../hooks/use-void-universe';
import type { VoidParticle } from '../../types';

// ============================================================================
// Camera Controller (lives inside R3F Canvas)
// ============================================================================

interface CameraTarget {
  position: THREE.Vector3;
  lookAt: THREE.Vector3;
}

interface CameraControllerProps {
  target: CameraTarget | null;
  onComplete: () => void;
}

/**
 * Smoothly animates the camera to a target position.
 * Must be rendered inside <Canvas>.
 */
function CameraController({ target, onComplete }: CameraControllerProps) {
  const { camera } = useThree();
  const progressRef = useRef(0);
  const startPos = useRef(new THREE.Vector3());
  const isAnimating = useRef(false);

  useEffect(() => {
    if (target) {
      startPos.current.copy(camera.position);
      progressRef.current = 0;
      isAnimating.current = true;
    }
  }, [target, camera]);

  useFrame((_, delta) => {
    if (!isAnimating.current || !target) return;

    progressRef.current += delta * 1.5; // ~0.67s duration
    const t = Math.min(progressRef.current, 1);
    // Ease-out cubic
    const ease = 1 - Math.pow(1 - t, 3);

    camera.position.lerpVectors(startPos.current, target.position, ease);

    if (t >= 1) {
      isAnimating.current = false;
      onComplete();
    }
  });

  return null;
}

// ============================================================================
// Search Overlay
// ============================================================================

interface SearchOverlayProps {
  query: string;
  onChange: (q: string) => void;
  onClose: () => void;
  matchCount: number;
}

function SearchOverlay({ query, onChange, onClose, matchCount }: SearchOverlayProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div
      style={{
        position: 'absolute',
        top: 16,
        left: '50%',
        transform: 'translateX(-50%)',
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        background: 'rgba(20,20,20,0.95)',
        border: '1px solid #2A2A2A',
        borderRadius: 8,
        padding: '8px 16px',
        zIndex: 20,
      }}
    >
      <span style={{ color: '#666', fontSize: 14 }}>/</span>
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Escape') onClose();
        }}
        placeholder="Search particles..."
        style={{
          background: 'transparent',
          border: 'none',
          outline: 'none',
          color: '#fff',
          fontFamily: 'Inter, sans-serif',
          fontSize: 14,
          width: 240,
        }}
      />
      <span style={{ color: '#666', fontSize: 11 }}>
        {matchCount > 0 ? `${matchCount} matches` : query ? 'No matches' : ''}
      </span>
    </div>
  );
}

// ============================================================================
// VoidUniverse (Main Component)
// ============================================================================

interface VoidUniverseProps {
  onClose: () => void;
}

/**
 * Full-screen 3D universe view.
 * Renders all particles, interest orbitals, and core glow.
 * Loaded via React.lazy() - Three.js bundle only imports when opened.
 */
export default function VoidUniverse({ onClose }: VoidUniverseProps) {
  const {
    universe,
    loading,
    error,
    selectedParticle,
    particleDetail,
    neighbors,
    loadUniverse,
    selectParticle,
  } = useVoidUniverse();

  const [cameraTarget, setCameraTarget] = useState<CameraTarget | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');

  // Load universe on mount
  useEffect(() => {
    loadUniverse();
  }, [loadUniverse]);

  // Fly camera to a 3D position (offset by 1.5 units along Z for viewing)
  const flyTo = useCallback((pos: [number, number, number]) => {
    setCameraTarget({
      position: new THREE.Vector3(pos[0], pos[1], pos[2] + 1.5),
      lookAt: new THREE.Vector3(pos[0], pos[1], pos[2]),
    });
  }, []);

  // Fly to home (overview)
  const flyHome = useCallback(() => {
    setCameraTarget({
      position: new THREE.Vector3(0, 0, 5),
      lookAt: new THREE.Vector3(0, 0, 0),
    });
  }, []);

  // Search filtering: compute which particles match
  const searchLower = searchQuery.toLowerCase();
  const searchMatches = new Set<string>();
  if (searchQuery && universe) {
    for (const p of universe.particles) {
      if (
        p.label.toLowerCase().includes(searchLower) ||
        p.source_type.toLowerCase().includes(searchLower)
      ) {
        searchMatches.add(`${p.layer}-${p.id}`);
      }
    }
  }

  // Keyboard shortcuts
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      // Don't handle keys when search input is focused
      if (searchOpen && e.key !== 'Escape') return;

      if (e.key === 'Escape') {
        if (searchOpen) {
          setSearchOpen(false);
          setSearchQuery('');
        } else if (selectedParticle) {
          selectParticle(null);
        } else {
          onClose();
        }
      } else if (e.key === 'r' || e.key === 'R') {
        loadUniverse();
      } else if (e.key === 'h' || e.key === 'H') {
        flyHome();
      } else if (e.key === 'f' || e.key === 'F') {
        if (selectedParticle) {
          flyTo(selectedParticle.position);
        }
      } else if (e.key === '/') {
        e.preventDefault();
        setSearchOpen(true);
      } else if (e.key >= '1' && e.key <= '9' && universe) {
        const idx = parseInt(e.key) - 1;
        if (idx < universe.interests.length) {
          flyTo(universe.interests[idx].position);
        }
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [onClose, selectedParticle, selectParticle, loadUniverse, flyTo, flyHome, searchOpen, universe]);

  const handleParticleSelect = useCallback(
    (particle: VoidParticle) => {
      selectParticle(particle);
      flyTo(particle.position);
    },
    [selectParticle, flyTo],
  );

  const handleDeselect = useCallback(() => {
    selectParticle(null);
  }, [selectParticle]);

  const handleCameraComplete = useCallback(() => {
    setCameraTarget(null);
  }, []);

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        background: '#0A0A0A',
        zIndex: 1000,
      }}
    >
      {/* Search overlay */}
      {searchOpen && (
        <SearchOverlay
          query={searchQuery}
          onChange={setSearchQuery}
          onClose={() => {
            setSearchOpen(false);
            setSearchQuery('');
          }}
          matchCount={searchMatches.size}
        />
      )}

      {/* HUD overlay */}
      <VoidHUD
        totalItems={universe?.total_items ?? 0}
        particleCount={universe?.particles.length ?? 0}
        interestCount={universe?.interests.length ?? 0}
        clusterCount={universe?.clusters.length ?? 0}
        loading={loading}
        onClose={onClose}
        onRefresh={loadUniverse}
      />

      {/* Selection panel */}
      {selectedParticle && (
        <VoidSelectionPanel
          particle={selectedParticle}
          detail={particleDetail}
          neighbors={neighbors}
          onClose={handleDeselect}
          onSelectNeighbor={handleParticleSelect}
        />
      )}

      {/* Error display */}
      {error && (
        <div
          style={{
            position: 'absolute',
            bottom: 60,
            left: '50%',
            transform: 'translateX(-50%)',
            background: 'rgba(239, 68, 68, 0.9)',
            color: '#fff',
            padding: '8px 16px',
            borderRadius: 6,
            fontSize: 12,
            zIndex: 10,
          }}
        >
          {error}
        </div>
      )}

      {/* 3D Canvas */}
      <Canvas
        camera={{
          position: [0, 0, 5],
          fov: 60,
          near: 0.01,
          far: 100,
        }}
        style={{ background: '#0A0A0A' }}
        onPointerMissed={handleDeselect}
      >
        {/* Ambient lighting */}
        <ambientLight intensity={0.3} />
        <pointLight position={[10, 10, 10]} intensity={0.5} />

        {/* Fog for depth */}
        <fog attach="fog" args={['#0A0A0A', 5, 30]} />

        {/* Camera animation controller */}
        <CameraController target={cameraTarget} onComplete={handleCameraComplete} />

        {/* Orbit controls */}
        <OrbitControls
          enableDamping
          dampingFactor={0.05}
          rotateSpeed={0.5}
          zoomSpeed={0.8}
          minDistance={0.5}
          maxDistance={50}
        />

        {/* Universe content */}
        {universe && (
          <>
            <VoidCore position={universe.core as [number, number, number]} />
            <VoidInterestOrbitals interests={universe.interests} />
            <VoidParticles
              particles={universe.particles}
              selectedId={selectedParticle?.id ?? null}
              selectedLayer={selectedParticle?.layer ?? null}
              onSelect={handleParticleSelect}
              searchFilter={searchQuery ? searchMatches : null}
            />
          </>
        )}

        {/* Loading indicator in 3D space */}
        {loading && !universe && (
          <mesh>
            <sphereGeometry args={[0.1, 16, 12]} />
            <meshBasicMaterial color="#D4AF37" wireframe />
          </mesh>
        )}
      </Canvas>
    </div>
  );
}
