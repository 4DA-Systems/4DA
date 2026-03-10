import type { StateCreator } from 'zustand';
import { cmd } from '../lib/commands';
import type { AppStore } from './types';
import type { PlaybookModule, PlaybookContent, PlaybookProgress } from '../types/playbook';
import type { PersonalizedLesson } from '../types/personalization';

export type StreetsTier = 'playbook';

export interface PlaybookSlice {
  playbookModules: PlaybookModule[];
  playbookContent: PlaybookContent | null;
  playbookProgress: PlaybookProgress | null;
  playbookLoading: boolean;
  playbookError: string | null;
  activeModuleId: string | null;
  personalizedLessons: Record<string, PersonalizedLesson>;
  streetsTier: StreetsTier;
  loadPlaybookModules: () => Promise<void>;
  loadPlaybookContent: (moduleId: string) => Promise<void>;
  loadPlaybookProgress: () => Promise<void>;
  markLessonComplete: (moduleId: string, lessonIdx: number) => Promise<void>;
  setActiveModuleId: (id: string | null) => void;
  loadPersonalizedContent: (moduleId: string, lessonIdx: number) => Promise<void>;
  loadPersonalizedContentBatch: (moduleId: string, lessonCount: number) => Promise<void>;
  loadStreetsTier: () => Promise<void>;
  activateStreetsLicense: (key: string) => Promise<boolean>;
}

export const createPlaybookSlice: StateCreator<AppStore, [], [], PlaybookSlice> = (set, get) => ({
  playbookModules: [],
  playbookContent: null,
  playbookProgress: null,
  playbookLoading: false,
  playbookError: null,
  activeModuleId: null,
  personalizedLessons: {},
  streetsTier: 'playbook',

  loadPlaybookModules: async () => {
    try {
      const modules = await cmd('get_playbook_modules');
      set({ playbookModules: modules });
    } catch (e) {
      set({ playbookError: String(e) });
    }
  },

  loadPlaybookContent: async (moduleId: string) => {
    set({ playbookLoading: true, playbookError: null, activeModuleId: moduleId });
    try {
      const content = await cmd('get_playbook_content', { moduleId });
      set({ playbookContent: content, playbookLoading: false });
    } catch (e) {
      set({ playbookError: String(e), playbookLoading: false });
    }
  },

  loadPlaybookProgress: async () => {
    try {
      const progress = await cmd('get_playbook_progress');
      set({ playbookProgress: progress });
    } catch (e) {
      set({ playbookError: String(e) });
    }
  },

  markLessonComplete: async (moduleId: string, lessonIdx: number) => {
    try {
      await cmd('mark_lesson_complete', { moduleId, lessonIdx });
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
      const lesson = await cmd('get_personalized_lesson', {
        moduleId,
        lessonIdx,
      }) as unknown as PersonalizedLesson;
      set({ personalizedLessons: { ...get().personalizedLessons, [key]: lesson } });

      // If LLM is available, trigger async hydration in the background
      if (lesson.depth.llm_pending) {
        cmd('hydrate_lesson_with_llm', { moduleId, lessonIdx }).catch((e) => {
          console.warn('LLM hydration failed (non-fatal):', e);
        });
      }
    } catch (e) {
      // Non-fatal: fallback to static content
      console.warn('Personalization failed, using static content:', e);
    }
  },

  loadPersonalizedContentBatch: async (moduleId: string, lessonCount: number) => {
    if (lessonCount <= 0) return;
    try {
      const requests: [string, number][] = Array.from({ length: lessonCount }, (_, i) => [moduleId, i]);
      const lessons = await cmd('get_personalized_lessons_batch', { requests }) as unknown as PersonalizedLesson[];
      const updated = { ...get().personalizedLessons };
      lessons.forEach((lesson, i) => {
        updated[`${moduleId}:${i}`] = lesson;
      });
      set({ personalizedLessons: updated });

      // Trigger LLM hydration for any lessons that need it
      for (let i = 0; i < lessons.length; i++) {
        if (lessons[i].depth.llm_pending) {
          cmd('hydrate_lesson_with_llm', { moduleId, lessonIdx: i }).catch((e) => {
            console.warn('LLM hydration failed (non-fatal):', e);
          });
        }
      }
    } catch (e) {
      console.warn('Batch personalization failed, falling back to static content:', e);
    }
  },

  loadStreetsTier: async () => {
    // Everyone gets the free playbook tier
    set({ streetsTier: 'playbook' });
  },

  activateStreetsLicense: async (_key: string) => {
    // No-op: community/cohort tiers removed. Kept for UI compat.
    return false;
  },
});
