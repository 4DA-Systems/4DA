import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { UserContext } from '../types';

export function useUserContext(onStatusChange?: (status: string) => void) {
  const [userContext, setUserContext] = useState<UserContext | null>(null);
  const [newInterest, setNewInterest] = useState('');
  const [newExclusion, setNewExclusion] = useState('');
  const [newTechStack, setNewTechStack] = useState('');
  const [newRole, setNewRole] = useState('');

  const setStatus = useCallback((status: string, duration = 2000) => {
    if (onStatusChange) {
      onStatusChange(status);
      if (duration > 0) {
        setTimeout(() => onStatusChange(''), duration);
      }
    }
  }, [onStatusChange]);

  const loadUserContext = useCallback(async () => {
    try {
      const ctx = await invoke<UserContext>('get_user_context');
      setUserContext(ctx);
      if (ctx.role) setNewRole(ctx.role);
    } catch (error) {
      console.log('Context not available:', error);
    }
  }, []);

  const addInterest = useCallback(async () => {
    if (!newInterest.trim()) return;
    try {
      await invoke('add_interest', { topic: newInterest.trim() });
      setNewInterest('');
      await loadUserContext();
      setStatus('Interest added');
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [newInterest, loadUserContext, setStatus]);

  const removeInterest = useCallback(async (topic: string) => {
    try {
      await invoke('remove_interest', { topic });
      await loadUserContext();
      setStatus('Interest removed');
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [loadUserContext, setStatus]);

  const addExclusion = useCallback(async () => {
    if (!newExclusion.trim()) return;
    try {
      await invoke('add_exclusion', { topic: newExclusion.trim() });
      setNewExclusion('');
      await loadUserContext();
      setStatus('Exclusion added');
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [newExclusion, loadUserContext, setStatus]);

  const removeExclusion = useCallback(async (topic: string) => {
    try {
      await invoke('remove_exclusion', { topic });
      await loadUserContext();
      setStatus('Exclusion removed');
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [loadUserContext, setStatus]);

  const addTechStack = useCallback(async () => {
    if (!newTechStack.trim()) return;
    try {
      await invoke('add_tech_stack', { technology: newTechStack.trim() });
      setNewTechStack('');
      await loadUserContext();
      setStatus('Technology added');
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [newTechStack, loadUserContext, setStatus]);

  const removeTechStack = useCallback(async (technology: string) => {
    try {
      await invoke('remove_tech_stack', { technology });
      await loadUserContext();
      setStatus('Technology removed');
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [loadUserContext, setStatus]);

  const updateRole = useCallback(async () => {
    try {
      await invoke('set_user_role', { role: newRole.trim() || null });
      await loadUserContext();
      setStatus('Role updated');
    } catch (error) {
      setStatus(`Error: ${error}`);
    }
  }, [newRole, loadUserContext, setStatus]);

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
