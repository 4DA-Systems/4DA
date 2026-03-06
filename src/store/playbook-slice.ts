import type { StateCreator } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppStore } from './types';
import type { PlaybookModule, PlaybookContent, PlaybookProgress } from '../types/playbook';
import type { PersonalizedLesson } from '../types/personalization';

export interface PlaybookSlice {
  playbookModules: PlaybookModule[];
  playbookContent: PlaybookContent | null;
  playbookProgress: PlaybookProgress | null;
  playbookLoading: boolean;
  playbookError: string | null;
  activeModuleId: string | null;
  personalizedLessons: Record<string, PersonalizedLesson>;
  loadPlaybookModules: () => Promise<void>;
  loadPlaybookContent: (moduleId: string) => Promise<void>;
  loadPlaybookProgress: () => Promise<void>;
  markLessonComplete: (moduleId: string, lessonIdx: number) => Promise<void>;
  setActiveModuleId: (id: string | null) => void;
  loadPersonalizedContent: (moduleId: string, lessonIdx: number) => Promise<void>;
}

export const createPlaybookSlice: StateCreator<AppStore, [], [], PlaybookSlice> = (set, get) => ({
  playbookModules: [],
  playbookContent: null,
  playbookProgress: null,
  playbookLoading: false,
  playbookError: null,
  activeModuleId: null,
  personalizedLessons: {},

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

  loadPersonalizedContent: async (moduleId: string, lessonIdx: number) => {
    const key = `${moduleId}:${lessonIdx}`;
    try {
      const lesson = await invoke<PersonalizedLesson>('get_personalized_lesson', {
        moduleId,
        lessonIdx,
      });
      set({ personalizedLessons: { ...get().personalizedLessons, [key]: lesson } });

      // If LLM is available, trigger async hydration in the background
      if (lesson.depth.llm_pending) {
        invoke('hydrate_lesson_with_llm', { moduleId, lessonIdx }).catch((e) => {
          console.warn('LLM hydration failed (non-fatal):', e);
        });
      }
    } catch (e) {
      // Non-fatal: fallback to static content
      console.warn('Personalization failed, using static content:', e);
    }
  },
});
