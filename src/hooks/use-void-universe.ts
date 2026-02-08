import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { VoidUniverse, VoidParticle } from '../types';

interface UseVoidUniverseReturn {
  universe: VoidUniverse | null;
  loading: boolean;
  error: string | null;
  selectedParticle: VoidParticle | null;
  particleDetail: Record<string, unknown> | null;
  neighbors: VoidParticle[];
  loadUniverse: () => Promise<void>;
  selectParticle: (particle: VoidParticle | null) => Promise<void>;
}

export function useVoidUniverse(): UseVoidUniverseReturn {
  const [universe, setUniverse] = useState<VoidUniverse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedParticle, setSelectedParticle] = useState<VoidParticle | null>(null);
  const [particleDetail, setParticleDetail] = useState<Record<string, unknown> | null>(null);
  const [neighbors, setNeighbors] = useState<VoidParticle[]>([]);

  const loadUniverse = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<VoidUniverse>('void_get_universe', {
        maxParticles: 5000,
      });
      setUniverse(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const selectParticle = useCallback(async (particle: VoidParticle | null) => {
    setSelectedParticle(particle);
    if (!particle) {
      setParticleDetail(null);
      setNeighbors([]);
      return;
    }

    try {
      const [detail, neighborResult] = await Promise.all([
        invoke<Record<string, unknown>>('void_get_particle_detail', {
          id: particle.id,
          layer: particle.layer,
        }),
        invoke<VoidParticle[]>('void_get_neighbors', {
          id: particle.id,
          layer: particle.layer,
          k: 8,
        }),
      ]);
      setParticleDetail(detail);
      setNeighbors(neighborResult);
    } catch {
      setParticleDetail(null);
      setNeighbors([]);
    }
  }, []);

  return {
    universe,
    loading,
    error,
    selectedParticle,
    particleDetail,
    neighbors,
    loadUniverse,
    selectParticle,
  };
}
