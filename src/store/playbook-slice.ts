import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore } from './types';
import type { PlaybookModule, PlaybookContent, PlaybookProgress } from '../types/playbook';

export interface PlaybookSlice {
  playbookModules: PlaybookModule[];
  playbookContent: PlaybookContent | null;
  playbookProgress: PlaybookProgress | null;
  playbookLoading: boolean;
  playbookError: string | null;
  activeModuleId: string | null;
  loadPlaybookModules: () => Promise<void>;
  loadPlaybookContent: (moduleId: string) => Promise<void>;
  loadPlaybookProgress: () => Promise<void>;
  markLessonComplete: (moduleId: string, lessonIdx: number) => Promise<void>;
  setActiveModuleId: (id: string | null) => void;
}

export const createPlaybookSlice: StateCreator<AppStore, [], [], PlaybookSlice> = (set, get) => ({
  playbookModules: [],
  playbookContent: null,
  playbookProgress: null,
  playbookLoading: false,
  playbookError: null,
  activeModuleId: null,

  loadPlaybookModules: async () => {
    try {
      const modules = await invoke<PlaybookModule[]>('get_playbook_modules');
      set({ playbookModules: modules });
    } catch (e) {
      set({ playbookError: String(e) });
    }
  },

  loadPlaybookContent: async (moduleId: string) => {
    set({ playbookLoading: true, playbookError: null, activeModuleId: moduleId });
    try {
      const content = await invoke<PlaybookContent>('get_playbook_content', { moduleId });
      set({ playbookContent: content, playbookLoading: false });
    } catch (e) {
      set({ playbookError: String(e), playbookLoading: false });
    }
  },

  loadPlaybookProgress: async () => {
    try {
      const progress = await invoke<PlaybookProgress>('get_playbook_progress');
      set({ playbookProgress: progress });
    } catch (e) {
      set({ playbookError: String(e) });
    }
  },

  markLessonComplete: async (moduleId: string, lessonIdx: number) => {
    try {
      await invoke('mark_lesson_complete', { moduleId, lessonIdx });
      // Reload progress
      get().loadPlaybookProgress();
    } catch (e) {
      set({ playbookError: String(e) });
    }
  },

  setActiveModuleId: (id) => set({ activeModuleId: id }),
});
