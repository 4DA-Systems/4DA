import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { UserContext, SuggestedInterest } from '../types';
import type { AppStore, UserContextSlice } from './types';

export const createUserContextSlice: StateCreator<AppStore, [], [], UserContextSlice> = (set, get) => ({
  userContext: null,
  suggestedInterests: [],
  newInterest: '',
  newExclusion: '',
  newTechStack: '',
  newRole: '',

  setNewInterest: (v) => set({ newInterest: v }),
  setNewExclusion: (v) => set({ newExclusion: v }),
  setNewTechStack: (v) => set({ newTechStack: v }),
  setNewRole: (v) => set({ newRole: v }),

  loadUserContext: async () => {
    try {
      const ctx = await invoke<UserContext>('get_user_context');
      set({ userContext: ctx });
      if (ctx.role) set({ newRole: ctx.role });
    } catch (error) {
      console.debug('Context not available:', error);
    }
  },

  loadSuggestedInterests: async () => {
    try {
      const suggestions = await invoke<SuggestedInterest[]>('ace_get_suggested_interests');
      set({ suggestedInterests: suggestions });
    } catch (error) {
      console.debug('Suggested interests not available:', error);
    }
  },

  addInterest: async () => {
    const { newInterest, loadUserContext, setSettingsStatus } = get();
    if (!newInterest.trim()) return;
    try {
      await invoke('add_interest', { topic: newInterest.trim() });
      set({ newInterest: '' });
      await loadUserContext();
      setSettingsStatus('Interest added');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  removeInterest: async (topic) => {
    const { loadUserContext, setSettingsStatus } = get();
    try {
      await invoke('remove_interest', { topic });
      await loadUserContext();
      setSettingsStatus('Interest removed');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  addExclusion: async () => {
    const { newExclusion, loadUserContext, setSettingsStatus } = get();
    if (!newExclusion.trim()) return;
    try {
      await invoke('add_exclusion', { topic: newExclusion.trim() });
      set({ newExclusion: '' });
      await loadUserContext();
      setSettingsStatus('Exclusion added');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  removeExclusion: async (topic) => {
    const { loadUserContext, setSettingsStatus } = get();
    try {
      await invoke('remove_exclusion', { topic });
      await loadUserContext();
      setSettingsStatus('Exclusion removed');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  addTechStack: async () => {
    const { newTechStack, loadUserContext, setSettingsStatus } = get();
    if (!newTechStack.trim()) return;
    try {
      await invoke('add_tech_stack', { technology: newTechStack.trim() });
      set({ newTechStack: '' });
      await loadUserContext();
      setSettingsStatus('Technology added');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  removeTechStack: async (technology) => {
    const { loadUserContext, setSettingsStatus } = get();
    try {
      await invoke('remove_tech_stack', { technology });
      await loadUserContext();
      setSettingsStatus('Technology removed');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },

  updateRole: async () => {
    const { newRole, loadUserContext, setSettingsStatus } = get();
    try {
      await invoke('set_user_role', { role: newRole.trim() || null });
      await loadUserContext();
      setSettingsStatus('Role updated');
      setTimeout(() => set({ settingsStatus: '' }), 2000);
    } catch (error) {
      setSettingsStatus(`Error: ${error}`);
    }
  },
});
