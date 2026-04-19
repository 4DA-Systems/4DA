// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useEffect } from 'react';
import { useAppStore } from '../store';

/**
 * User context hook — thin wrapper around Zustand store.
 * All state lives in the store; this hook adds the init-load effect.
 */
export function useUserContext(_onStatusChange?: (status: string) => void) {
  const userContext = useAppStore(s => s.userContext);
  const newInterest = useAppStore(s => s.newInterest);
  const setNewInterest = useAppStore(s => s.setNewInterest);
  const newExclusion = useAppStore(s => s.newExclusion);
  const setNewExclusion = useAppStore(s => s.setNewExclusion);
  const newTechStack = useAppStore(s => s.newTechStack);
  const setNewTechStack = useAppStore(s => s.setNewTechStack);
  const newRole = useAppStore(s => s.newRole);
  const setNewRole = useAppStore(s => s.setNewRole);
  const loadUserContext = useAppStore(s => s.loadUserContext);
  const addInterest = useAppStore(s => s.addInterest);
  const removeInterest = useAppStore(s => s.removeInterest);
  const addExclusion = useAppStore(s => s.addExclusion);
  const removeExclusion = useAppStore(s => s.removeExclusion);
  const addTechStack = useAppStore(s => s.addTechStack);
  const removeTechStack = useAppStore(s => s.removeTechStack);
  const updateRole = useAppStore(s => s.updateRole);

  useEffect(() => {
    loadUserContext();
  }, [loadUserContext]);

  return {
    userContext,
    newInterest,
    setNewInterest,
    newExclusion,
    setNewExclusion,
    newTechStack,
    setNewTechStack,
    newRole,
    setNewRole,
    loadUserContext,
    addInterest,
    removeInterest,
    addExclusion,
    removeExclusion,
    addTechStack,
    removeTechStack,
    updateRole,
  };
}
